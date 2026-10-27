//! Normal native HTTP adapter tests; fictional responses are not hosted Auth proof.
use super::government_export_tests::{access_json,client};
use super::*;
use crate::payroll_government::{bir_file::{self as bir,builder},registration};
use std::{io::{BufRead,BufReader,Read,Write},net::TcpListener,path::PathBuf,time::Duration};

fn fixture()->(Snapshot,PathBuf,Vec<u8>) {
    let mut data=DemoStore::new().data;
    data.company.id=Uuid::from_u128(1);data.access.company_id=data.company.id;
    data.access.user_id=Uuid::from_u128(2);data.mode=Mode::Supabase;
    let root=std::env::temp_dir().join(format!("hris-bir-build-http-{}",Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let mut full=vec![String::new();49];
    for &column in &bir::MONEY_COLUMNS {full[column]="0.00".into();}
    for (column,value) in [(6,"234567890"),(7,"0000"),(8,"FICTIONAL FAMILY"),
        (9,"FICTIONAL GIVEN"),(11,"NCR"),(23,"01/01/2026"),(24,"12/31/2026"),
        (44,"FILIPINO"),(45,"R"),(46,"NA"),(47,"N")] {full[column]=value.into();}
    let bytes=crate::imports::csv(builder::headers(bir::Schedule::One),[full[6..].to_vec()]).unwrap();
    std::fs::write(root.join("declarations.csv"),&bytes).unwrap();
    (data,root,bytes)
}
fn responses(data:&Snapshot)->Vec<(&'static str,Value,bool)> {
    let access=access_json(&data.access);
    let registration=serde_json::to_value(registration::preview(data.company.id,None)).unwrap();
    [false,false,true,false,false,true,false,false].into_iter().map(|r|
        if r {("/rest/v1/rpc/government_registration",registration.clone(),false)}
        else {("/rest/v1/rpc/my_hris_access",access.clone(),false)}).collect()
}
fn server(responses:Vec<(&'static str,Value,bool)>)->(String,std::thread::JoinHandle<()>) {
    let listener=TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let url=format!("http://{}",listener.local_addr().unwrap());
    let handle=std::thread::spawn(move||{
        for (path,payload,denied) in responses {
            let end=std::time::Instant::now()+Duration::from_secs(6);
            let mut socket=loop {
                match listener.accept() {
                    Ok((socket,_))=>break socket,
                    Err(error) if error.kind()==std::io::ErrorKind::WouldBlock=>{
                        assert!(std::time::Instant::now()<end,"Builder request did not arrive: {path}");
                        std::thread::sleep(Duration::from_millis(2));
                    }
                    Err(error)=>panic!("HTTP accept: {error}"),
                }
            };
            socket.set_nonblocking(false).unwrap();
            socket.set_read_timeout(Some(Duration::from_secs(2))).unwrap();
            let mut reader=BufReader::new(socket.try_clone().unwrap());
            let mut line=String::new();reader.read_line(&mut line).unwrap();
            assert_eq!(line.split_whitespace().nth(1),Some(path));
            let mut length=0usize;
            loop {
                line.clear();reader.read_line(&mut line).unwrap();
                if line=="\r\n"{break;}
                if let Some((key,value))=line.split_once(':') {
                    if key.eq_ignore_ascii_case("content-length"){length=value.trim().parse().unwrap();}
                }
            }
            assert!(length<1024);
            let mut body=vec![0;length];reader.read_exact(&mut body).unwrap();
            assert_eq!(serde_json::from_slice::<Value>(&body).unwrap(),if path.ends_with("government_registration"){
                json!({"p_employee_id":null})
            }else{json!({})},"Declaration fields must never be sent to Supabase.");
            let bytes=serde_json::to_vec(&payload).unwrap();
            let status=if denied{"403 Forbidden"}else{"200 OK"};
            write!(socket,"HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",bytes.len()).unwrap();
            socket.write_all(&bytes).unwrap();
        }
    });
    (url,handle)
}
fn action(root:&Path,hash:Option<String>,destination:bool,reviewed:bool)->builder::Action {
    builder::Action::Prepare {selection:builder::Selection{schedule:bir::Schedule::One,year:2026},
        source:root.join("declarations.csv"),expected_basis:hash,
        destination:destination.then(||root.join("0123456780000123120261604C.DAT")),reviewed}
}
fn clean(root:&Path) {
    for name in ["declarations.csv","0123456780000123120261604C.DAT","blank.xlsx"] {
        let path=root.join(name);if path.exists(){std::fs::remove_file(path).unwrap();}
    }
    std::fs::remove_dir(root).unwrap();
}

#[test]
fn builder_native_hr_admin_preview_and_new_file_with_only_access_registration_requests() {
    for role in [crate::access::Role::Administrator,crate::access::Role::HrGeneralist] {
        let (mut data,root,before)=fixture();data.access.role=role;
        let mut requests=responses(&data);requests.extend(responses(&data));
        let (url,handle)=server(requests);let mut client=client(url,&data);
        let preview=client.government_bir_build(&data,&action(&root,None,false,false)).unwrap().unwrap();
        let saved=client.government_bir_build(&data,&action(&root,Some(preview.basis_sha256.clone()),true,true)).unwrap().unwrap();
        assert_eq!(saved.output_sha256,preview.output_sha256);
        let bytes=std::fs::read(root.join(saved.file_name)).unwrap();
        assert!(std::str::from_utf8(&bytes).unwrap().starts_with("H1604C,012345678,0000,"));
        assert_eq!(std::fs::read(root.join("declarations.csv")).unwrap(),before);
        handle.join().unwrap();clean(&root);
    }
}

#[test]
fn builder_native_employee_manager_denied_before_network_or_file_reads() {
    for role in [crate::access::Role::Employee,crate::access::Role::Manager] {
        let (mut data,root,_)=fixture();data.access.role=role;
        let mut client=client("http://127.0.0.1:9".into(),&data);
        assert!(client.government_bir_build(&data,&action(&root,None,false,false)).is_err());
        assert!(client.government_bir_build(&data,&builder::Action::Template{schedule:bir::Schedule::One,destination:root.join("blank.xlsx")}).is_err());
        assert!(!root.join("blank.xlsx").exists());clean(&root);
    }
}

#[test]
fn builder_native_review_and_preview_required_before_save() {
    for reviewed in [false,true] {
        let (data,root,_)=fixture();
        let (url,handle)=server(responses(&data)[..1].to_vec());
        let mut client=client(url,&data);
        assert!(client.government_bir_build(&data,&action(&root,None,true,reviewed)).is_err());
        assert!(!root.join("0123456780000123120261604C.DAT").exists());
        handle.join().unwrap();clean(&root);
    }
}

#[test]
fn builder_native_stale_source_or_registration_does_not_write() {
    for registration_change in [false,true] {
        let (data,root,_)=fixture();
        let mut initial=responses(&data);
        let mut next=responses(&data);
        if registration_change {
            let mut changed=registration::preview(data.company.id,None);
            changed.record.as_mut().unwrap().details.insert("bir_branch".into(),"0001".into());
            changed.history[0].after_record=changed.record.as_ref().unwrap().clone();
            let value=serde_json::to_value(changed).unwrap();
            next[2].1=value;
        }
        initial.extend(next[..4].iter().cloned());
        let (url,handle)=server(initial);let mut client=client(url,&data);
        let preview=client.government_bir_build(&data,&action(&root,None,false,false)).unwrap().unwrap();
        if !registration_change {
            let mut bytes=std::fs::read(root.join("declarations.csv")).unwrap();bytes.extend(b"\r\n");
            std::fs::write(root.join("declarations.csv"),bytes).unwrap();
        }
        assert!(client.government_bir_build(&data,&action(&root,Some(preview.basis_sha256),true,true)).is_err());
        assert!(!root.join("0123456780000123120261604C.DAT").exists());
        handle.join().unwrap();clean(&root);
    }
}

#[test]
fn builder_native_last_access_revocation_or_during_render_registration_change_prevents_write() {
    for registration_change in [false,true] {
        let (data,root,_)=fixture();
        let (url,handle)=server(responses(&data));let mut client1=client(url,&data);
        let preview=client1.government_bir_build(&data,&action(&root,None,false,false)).unwrap().unwrap();
        handle.join().unwrap();
        let mut next=responses(&data);
        if registration_change {
            let mut changed=registration::preview(data.company.id,None);
            changed.record.as_mut().unwrap().details.insert("bir_branch".into(),"0001".into());
            changed.history[0].after_record=changed.record.as_ref().unwrap().clone();
            next[5].1=serde_json::to_value(changed).unwrap();next.truncate(7);
        }else{next[7].1=json!({"message":"Fictional membership revoked"});next[7].2=true;}
        let (url,handle)=server(next);let mut client2=client(url,&data);
        assert!(client2.government_bir_build(&data,&action(&root,Some(preview.basis_sha256),true,true)).is_err());
        assert!(!root.join("0123456780000123120261604C.DAT").exists());
        handle.join().unwrap();clean(&root);
    }
}

#[test]
fn builder_native_blank_template_checks_live_access_without_employer_edits() {
    let (data,root,_)=fixture();
    let (url,handle)=server(responses(&data)[..2].to_vec());let mut client=client(url,&data);
    assert!(client.government_bir_build(&data,&builder::Action::Template{schedule:bir::Schedule::One,destination:root.join("blank.xlsx")}).unwrap().is_none());
    assert!(std::fs::read(root.join("blank.xlsx")).unwrap().starts_with(b"PK"));
    handle.join().unwrap();clean(&root);
}
