"""Collect pinned local dependency licenses for the private Android package."""
from pathlib import Path
import json,subprocess,xml.etree.ElementTree as E,zipfile
ROOT=Path(__file__).resolve().parents[1]
APP=ROOT/"employee-android"
def main():
    command=["cargo","metadata","--offline","--locked","--format-version","1","--filter-platform","aarch64-linux-android","--manifest-path",str(APP/"rust/Cargo.toml")]
    metadata=json.loads(subprocess.check_output(command,cwd=ROOT))
    nodes={n["id"]:n for n in metadata["resolve"]["nodes"]}; used=set(); todo=[metadata["resolve"]["root"]]
    while todo:
        identity=todo.pop()
        if identity in used:continue
        used.add(identity);todo.extend(n["pkg"]for n in nodes[identity]["deps"])
    sections=["HRIS Employee Android: third-party notices", "Unmodified dependency source is available at the version-specific crate/repository URLs below. This includes source for MPL-2.0-covered webpki-roots. Project source is delivered separately with this private prototype."]
    texts=set();rust_count=0
    for package in sorted(metadata["packages"],key=lambda p:(p["name"],p["version"])):
        if package["id"]not in used or not package.get("source"):continue
        license=package.get("license") or "See upstream license"
        sections.append(f"Rust: {package['name']} {package['version']} | {license} | https://crates.io/crates/{package['name']}/{package['version']}")
        directory=Path(package["manifest_path"]).parent;rust_count+=1
        paths=set(directory.glob("LICENSE*"))|set(directory.glob("COPYING*"))|set(directory.glob("NOTICE*"))
        if package.get("license_file"):paths.add(directory/package["license_file"])
        for path in sorted(paths):
            if not path.is_file()or path.stat().st_size>300000:continue
            body=path.read_text(encoding="utf-8",errors="replace").strip()
            if body and body not in texts:sections.append(f"{package['name']}/{path.name}\n{body}");texts.add(body)
    cache=ROOT/"work/android-gradle-cache/caches/modules-2/files-2.1";maven_count=0
    for line in (APP/"app/gradle.lockfile").read_text(encoding="utf-8").splitlines():
        if "="not in line or line.startswith("#")or "RuntimeClasspath"not in line:continue
        coordinate=line.split("=",1)[0].split(":")
        if len(coordinate)!=3:continue
        group,name,version=coordinate;directory=cache/group/name/version
        poms=list(directory.glob("*/*.pom"));assert poms,"Pinned Maven attribution missing: "+name
        tree=E.parse(poms[0]);ns={"m":"http://maven.apache.org/POM/4.0.0"}
        licenses=tree.findall(".//m:licenses/m:license",ns)
        label="; ".join((x.findtext("m:name",default="",namespaces=ns)+" "+x.findtext("m:url",default="",namespaces=ns)).strip()for x in licenses)
        sections.append("Android: "+":".join(coordinate)+" | "+(label or "See upstream POM"));maven_count+=1
        for archive in list(directory.glob("*/*.jar"))+list(directory.glob("*/*.aar")):
            with zipfile.ZipFile(archive)as z:
                for item in z.infolist():
                    upper=item.filename.upper()
                    if upper.startswith("META-INF/")and any(upper.endswith(x)for x in ["LICENSE","LICENSE.TXT","NOTICE","NOTICE.TXT"])and item.file_size<300000:
                        body=z.read(item).decode("utf-8",errors="replace").strip()
                        if body and body not in texts:sections.append(name+"/"+item.filename+"\n"+body);texts.add(body)
    apache=(ROOT/"work/android-tools/gradle-9.8.2/LICENSE").read_text(encoding="utf-8")
    sections.append("Apache License 2.0 (AndroidX, Kotlin and other indicated components)\n"+apache)
    asset=APP/"app/src/main/assets/THIRD-PARTY-NOTICES.txt";asset.write_text("\n\n".join(sections)+"\n",encoding="utf-8")
    print(f"Android dependency attribution: {rust_count} Rust packages, {maven_count} Maven entries; {asset.stat().st_size} bytes")
if __name__=="__main__":main()
