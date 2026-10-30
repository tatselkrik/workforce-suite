/* Test APK only: fictional rendering data. Never authenticates or writes to the company backend. */
(() => {
  try {
    window.__designWrites = 0;
    const asof = today(), zero = '2026-10-30T00:00:00Z';
    const service = {schema_version:1,requests:[]};
    api = async (path, body) => {
      if (body !== undefined) { window.__designWrites++; throw Error('Design fixture blocks all writes'); }
      if (path === '/api/services') return service;
      throw Error('Unexpected design-fixture API read');
    };
    state.me = {csrf:'fictional-rendering-only',email:'fictional@example.invalid',
      company:{id:'00000000-0000-0000-0000-000000000001',name:'HRIS Company',timezone:'Asia/Manila'},
      employment:{employee_code:'DEMO-014',full_name:'Alex Rivera',department:'Operations',job_title:'Customer Support Specialist',joined_on:'2026-01-15',employment_type:'Full-time',work_location:'Main office',reports_to:'Jordan Cruz',annual_leave_days:15},
      contacts:{phone:'',personal_email:'',address:'',emergency_name:'',emergency_relationship:'',emergency_phone:'',emergency_alternate_phone:''}};
    state.csrf = 'fictional-rendering-only';
    state.attendance = [{id:'00000000-0000-0000-0000-000000000002',work_date:asof,checked_in_at:zero,checked_out_at:null}];
    state.payslips = [{id:'00000000-0000-0000-0000-000000000003',month:'2026-09-01',net:2800000}];
    pictureState.logo = null; pictureState.photo = null; pictureState.preview = null;
    state.slip = null; state.page = 'home'; state.busy = false;
    selfState.context = {asof,workweek:[1,2,3,4,5],holidays:[],types:[{code:'annual',name:'Annual leave',paid:true,unlimited:false,balance:{available_units:30,used_units:0,reserved_units:0}}]};
    selfState.leave = [{id:'00000000-0000-0000-0000-000000000004',type_name:'Annual leave',status:'pending',starts_on:asof,ends_on:asof,day_part:'full',charged_units:2,reason:'Fictional personal appointment',approvals:[]}];
    selfState.breaks = null;
    timeState.context = {work_date:asof,timezone:'Asia/Manila',attendance:state.attendance[0],candidate_minutes:90,current_approved_minutes:0,correction_pending:false,overtime_blocked:false};
    timeState.requests = [];
    serviceState.data = service;
    inboxState.data = {unread_count:1,total_items:1,unread_only:false,generated_at:zero,offset:0,limit:20,has_more:false,items:[{key:'fictional-payslip',title:'Your payslip is ready',detail:'Open your approved September payslip.',date:asof,read:false,urgent:false,target:{page:'pay'}}]};
    accountState.data = {email:'fictional@example.invalid',confirmation_pending:false,pending_email:null};
    nativeAlertRequest = async () => ({enabled:false,permission:true});
    $('company-name').textContent = $('login-company').textContent = 'HRIS Company';
    $('login-initials').textContent = 'HC';
    render();
    window.__designSlip = {id:state.payslips[0].id,company_name:'HRIS Company',month:'2026-09-01',rule_version:'Fictional layout example',employee:{employee_code:'DEMO-014',employee_name:'Alex Rivera',department:'Operations'},totals:{basic:3000000,paid_leave:0,unworked_holiday:0,premium:0,overtime:0,night:0,allowances:250000,gross:3250000,reimbursement:0,sss_employee:150000,philhealth_employee:75000,pagibig_employee:20000,withholding:200000,voluntary_deductions:5000,advances:0,net:2800000},components:[],adjustments:[]};
    return JSON.stringify({ok:true});
  } catch (e) { return JSON.stringify({ok:false,error:String(e)}); }
})();
