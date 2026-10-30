"""Build the private Android employee package; never print deployment/signing secrets."""
from pathlib import Path
import argparse, base64, ctypes, hashlib, json, os, re, secrets, shutil, subprocess
ROOT = Path(__file__).resolve().parents[1]
APP = ROOT / "employee-android"
SDK = Path(os.environ["LOCALAPPDATA"]) / "Android/Sdk"
JAVA = Path(os.environ.get("JAVA_HOME", "C:/Program Files/Android/Android Studio/jbr"))
PRIVATE = ROOT / "deployment-private/android-employee"
class Blob(ctypes.Structure):
    _fields_ = [("size", ctypes.c_uint32), ("data", ctypes.POINTER(ctypes.c_ubyte))]
def protect(raw, decrypt=False):
    data = (ctypes.c_ubyte * len(raw)).from_buffer_copy(raw); source = Blob(len(raw), data); output = Blob()
    crypt = ctypes.WinDLL("crypt32", use_last_error=True); kernel = ctypes.WinDLL("kernel32", use_last_error=True)
    function = crypt.CryptUnprotectData if decrypt else crypt.CryptProtectData
    function.argtypes = [ctypes.POINTER(Blob), ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_void_p, ctypes.c_uint32, ctypes.POINTER(Blob)]
    function.restype = ctypes.c_int; kernel.LocalFree.argtypes = [ctypes.c_void_p]
    if not function(ctypes.byref(source), None, None, None, None, 1, ctypes.byref(output)): raise OSError("Private signing state unavailable")
    try: return ctypes.string_at(output.data, output.size)
    finally: kernel.LocalFree(output.data)
def command(args, env):
    result = subprocess.run([str(x) for x in args], cwd=ROOT, env=env)
    if result.returncode: raise SystemExit(result.returncode)
def signing(env):
    PRIVATE.mkdir(parents=True, exist_ok=True); key = PRIVATE / "employee-release.p12"; password_file = PRIVATE / "signing-password.dpapi"
    if key.exists():
        if not password_file.exists(): raise RuntimeError("Keep the existing signing key; restore its private password file")
        password = protect(password_file.read_bytes(), True).decode()
    else:
        if password_file.exists(): raise RuntimeError("Review incomplete signing state before replacing anything")
        password = secrets.token_urlsafe(48); password_file.write_bytes(protect(password.encode()))
        env["HRIS_ANDROID_SIGNING_PASSWORD"] = password
        command([JAVA / "bin/keytool.exe", "-genkeypair", "-noprompt", "-keystore", key, "-storetype", "PKCS12", "-alias", "hris-employee", "-keyalg", "RSA", "-keysize", "3072", "-validity", "10000", "-dname", "CN=HRIS Employee Prototype,O=Private prototype", "-storepass:env", "HRIS_ANDROID_SIGNING_PASSWORD", "-keypass:env", "HRIS_ANDROID_SIGNING_PASSWORD"], env)
    env["HRIS_ANDROID_SIGNING_PASSWORD"] = password; env["HRIS_ANDROID_KEYSTORE"] = str(key)
def main():
    parser = argparse.ArgumentParser(); parser.add_argument("--skip-native", action="store_true"); parser.add_argument("--instrumentation", action="store_true"); parser.add_argument("--release", action="store_true"); parser.add_argument("--candidate", action="store_true"); parser.add_argument("--refresh-locks", action="store_true"); args = parser.parse_args()
    env = os.environ.copy(); env.update(JAVA_HOME=str(JAVA), GRADLE_USER_HOME=str(ROOT / "work/android-gradle-cache"), CARGO_TARGET_DIR=str(ROOT / "work/android-rust-target"), CARGO_INCREMENTAL="0", CARGO_PROFILE_DEV_DEBUG="0", CARGO_PROFILE_TEST_DEBUG="0")
    bin = SDK / "ndk/30.0.16248370/toolchains/llvm/prebuilt/windows-x86_64/bin"
    for target, abi in [("aarch64-linux-android", "arm64-v8a"), ("x86_64-linux-android", "x86_64")]:
        name = target.replace("-", "_"); env["CARGO_TARGET_" + name.upper() + "_LINKER"] = str(bin / (target + "26-clang.cmd")); env["CC_" + name] = env["CARGO_TARGET_" + name.upper() + "_LINKER"]; env["AR_" + name] = str(bin / "llvm-ar.exe")
        source = ROOT / "work/android-rust-target" / target / "release/libhris_employee_android.so"
        if not args.skip_native: command(["cargo", "build", "--locked", "--manifest-path", APP / "rust/Cargo.toml", "--target", target, "--release"], env)
        if not source.exists() or source.stat().st_size < 100000: raise RuntimeError("Verified native library missing for " + abi)
        destination = APP / "app/src/main/jniLibs" / abi; destination.mkdir(parents=True, exist_ok=True); shutil.copyfile(source, destination / source.name)
    git = ["git", "--work-tree=" + str(ROOT), "--git-dir=" + str(ROOT / ".git")]
    revision = subprocess.check_output(git + ["rev-parse", "HEAD"], cwd=ROOT, text=True).strip()
    clean = not subprocess.check_output(git + ["status", "--porcelain"], cwd=ROOT, text=True).strip()
    if args.release and not args.candidate and not clean: raise RuntimeError("Commit release-affecting inputs before the verified release build")
    connected = False
    acceptance = ROOT / "work/android-employee/push-acceptance-results.json"
    rollout = ROOT / "work/android-employee/push-rollout-verified.json"
    if acceptance.exists() and rollout.exists():
        qa = json.loads(acceptance.read_text(encoding="utf-8")); delivery = json.loads(rollout.read_text(encoding="utf-8"))
        connected = qa.get("all_passed") is True and qa.get("proof", {}).get("actual_background_fcm_received") is True and delivery.get("all_passed") is True and delivery.get("dispatch_enabled") is True
        for evidence in ["direct-tick-probe-results.json", "push-visual-results.json"]:
            path = ROOT / "work/android-employee" / evidence
            connected = connected and path.exists() and json.loads(path.read_text(encoding="utf-8")).get("all_passed") is True
    if args.release and not args.candidate and not connected: raise RuntimeError("Finish verified background delivery and the protected rollout before the release")
    version_match = re.search(r'versionCode = (\d+); versionName = "([0-9]+\.[0-9]+\.[0-9]+(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?)"', (APP / "app/build.gradle.kts").read_text(encoding="utf-8"))
    if not version_match: raise RuntimeError("Resolve the pinned Android version before packaging")
    assets = APP / "app/src/main/assets";
    (assets / "BUILD.json").write_text(json.dumps({"application_id":"io.github.tatselkrik.hrisemployee","version":version_match.group(2),"version_code":int(version_match.group(1)),"source_commit":revision,"source_worktree_clean":clean,"channel":"private local prototype","background_push_connected":connected,"physical_phone_verified":False},indent=2)+"\n",encoding="utf-8")
    source = Path(os.environ["LOCALAPPDATA"]) / "Programs/HRIS Desktop/hris-connection.json"
    profile = json.loads(source.read_text(encoding="utf-8-sig")); reference = (ROOT / "supabase/.temp/project-ref").read_text(encoding="utf-8").strip()
    if set(profile) != {"url", "publishable_key"} or not profile["publishable_key"].startswith("sb_publishable_") or profile["url"].rstrip("/") != "https://" + reference + ".supabase.co": raise RuntimeError("Company deployment identity mismatch")
    shutil.copyfile(source, assets / "hris-connection.json")
    branding = ROOT / "work/portal-deployment-0.1.8/HRIS-Employee-Portal/hris-branding.json"
    if branding.exists(): shutil.copyfile(branding, assets / "hris-branding.json")
    if not (APP / "app/google-services.json").exists(): raise RuntimeError("Prepare the authorized Firebase Android configuration first")
    firebase = json.loads((APP / "app/google-services.json").read_text(encoding="utf-8"))
    firebase_guard = (PRIVATE / "FIREBASE-AUTHORIZATION.md").read_text(encoding="utf-8")
    if "`" + firebase["project_info"]["project_id"] + "`" not in firebase_guard or firebase["client"][0]["client_info"]["android_client_info"]["package_name"] != "io.github.tatselkrik.hrisemployee": raise RuntimeError("Firebase deployment target mismatch")
    signing(env)
    command([ROOT / "work/android-tools/gradle-9.8.2/bin/gradle.bat", "-p", APP, ":app:dependencies", "--configuration", "releaseRuntimeClasspath" if args.release else "debugRuntimeClasspath", *(["--write-locks"] if args.refresh_locks else []), "--no-daemon"], env)
    command([os.sys.executable, ROOT / "tools/android-employee-notices.py"], env)
    tasks = [":app:assembleRelease", ":app:lintRelease"] if args.release else [":app:assembleDebug", ":app:testDebugUnitTest", ":app:lintDebug"]
    if args.instrumentation: tasks.append(":app:assembleDebugAndroidTest")
    command([ROOT / "work/android-tools/gradle-9.8.2/bin/gradle.bat", "-p", APP, *tasks, *(["--write-locks"] if args.refresh_locks else []), "--no-daemon"], env)
    if args.release and not args.candidate and subprocess.check_output(git + ["status", "--porcelain"], cwd=ROOT, text=True).strip(): raise RuntimeError("Release inputs changed during the build; commit and rebuild before delivery")
    print("Private Android build completed; deployment and signing credentials were not logged")
if __name__ == "__main__": main()
