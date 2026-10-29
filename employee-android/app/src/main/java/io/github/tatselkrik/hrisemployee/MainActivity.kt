package io.github.tatselkrik.hrisemployee

import android.annotation.SuppressLint
import android.app.AlertDialog
import android.content.Intent
import android.net.Uri
import android.os.Bundle
import android.print.PrintManager
import android.view.ViewGroup
import android.view.WindowManager
import android.webkit.CookieManager
import android.webkit.JsResult
import android.webkit.ValueCallback
import android.webkit.WebChromeClient
import android.webkit.WebResourceRequest
import android.webkit.WebSettings
import android.webkit.WebStorage
import android.webkit.WebView
import android.webkit.WebViewClient
import android.webkit.RenderProcessGoneDetail
import androidx.activity.ComponentActivity
import androidx.activity.OnBackPressedCallback
import androidx.activity.compose.setContent
import androidx.activity.result.contract.ActivityResultContracts
import androidx.compose.foundation.layout.*
import androidx.compose.foundation.isSystemInDarkTheme
import androidx.compose.material3.*
import androidx.compose.runtime.*
import androidx.compose.ui.Modifier
import androidx.compose.ui.graphics.Color
import androidx.compose.ui.unit.dp
import androidx.compose.ui.viewinterop.AndroidView
import androidx.webkit.WebMessageCompat
import androidx.webkit.WebViewCompat
import androidx.webkit.WebViewFeature
import org.json.JSONObject
import java.io.File
import java.lang.ref.WeakReference
import java.util.concurrent.Executors

/** No credentials, session tokens, employee records or WebView state are persisted to disk.
 * The retained view is constructed with Application context; Activity access is weak.
 */
@SuppressLint("StaticFieldLeak")
internal object EmployeeProcess {
    val worker = Executors.newSingleThreadExecutor()
    var origin: String? = null
    var view: WebView? = null
    var activity = WeakReference<MainActivity>(null)
}

class MainActivity : ComponentActivity() {
    internal var browser: WebView? = null
    internal var origin: String? = null
    private var pendingPhoto: ValueCallback<Array<Uri>>? = null
    private var exportBytes: ByteArray? = null
    private val photoPicker = registerForActivityResult(ActivityResultContracts.GetContent()) { uri ->
        pendingPhoto?.onReceiveValue(uri?.let { arrayOf(it) }); pendingPhoto = null
    }
    private val exportPicker = registerForActivityResult(ActivityResultContracts.CreateDocument("text/csv")) { uri ->
        val bytes = exportBytes; exportBytes = null
        if (uri != null && bytes != null) EmployeeProcess.worker.execute {
            try { (contentResolver.openOutputStream(uri, "w") ?: error("Document unavailable")).use { it.write(bytes) } }
            catch (_: Exception) { runOnUiThread { message("The file could not be saved. Try exporting again.") } }
        }
    }
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        if (!BuildConfig.DEBUG) window.addFlags(WindowManager.LayoutParams.FLAG_SECURE)
        EmployeeProcess.activity = WeakReference(this)
        var serviceOrigin by mutableStateOf(EmployeeProcess.origin)
        var failure by mutableStateOf<String?>(null)
        setContent {
            MaterialTheme(colorScheme = if (isSystemInDarkTheme()) darkColorScheme(primary = Color(0xFF80D4C4), background = Color(0xFF17242E), surface = Color(0xFF17242E)) else lightColorScheme(primary = Color(0xFF087E7E), background = Color.White, surface = Color.White)) {
                Surface(Modifier.fillMaxSize()) {
                    Column(Modifier.fillMaxSize().safeDrawingPadding().imePadding()) {
                        Row(Modifier.fillMaxWidth().height(44.dp).padding(horizontal = 12.dp), horizontalArrangement = Arrangement.SpaceBetween) {
                            Text("HRIS Employee", style = MaterialTheme.typography.titleMedium, modifier = Modifier.padding(top = 9.dp))
                            TextButton(onClick = { notifications() }) { Text("Device alerts") }
                        }
                        when {
                            failure != null -> Column(Modifier.padding(24.dp)) { Text(failure!!); Button(onClick = { recreate() }) { Text("Try again") } }
                            serviceOrigin == null -> Column(Modifier.padding(24.dp)) { CircularProgressIndicator(); Text("Opening your company workspace…") }
                            else -> AndroidView(factory = { protectedBrowser(serviceOrigin!!) }, modifier = Modifier.fillMaxWidth().weight(1f))
                        }
                    }
                }
            }
        }
        onBackPressedDispatcher.addCallback(this, object : OnBackPressedCallback(true) {
            override fun handleOnBackPressed() {
                browser?.evaluateJavascript("Boolean((typeof state!=='undefined' && (state.dirty||state.pictureDirty||state.securityDirty||state.busy)) || (typeof timeState!=='undefined' && timeState.dirtyKind) || (typeof serviceState!=='undefined' && serviceState.dirty))") { answer ->
                    if (answer == "true") AlertDialog.Builder(this@MainActivity).setMessage("Discard unsaved edits and close HRIS Employee?").setNegativeButton("Keep editing", null).setPositiveButton("Discard and close") { _, _ -> finish() }.show()
                    else finish()
                } ?: finish()
            }
        })
        if (serviceOrigin == null) {
            // A new process never revives a previous browser cookie/session.
            WebStorage.getInstance().deleteAllData()
            CookieManager.getInstance().removeAllCookies {
                EmployeeProcess.worker.execute {
                    try {
                        val dir = File(filesDir, "company").apply { mkdirs() }
                        for (name in listOf("hris-connection.json", "hris-branding.json")) {
                            try { assets.open(name).use { input -> File(dir, name).outputStream().use { input.copyTo(it) } } }
                            catch (e: Exception) { if (name == "hris-connection.json") throw e }
                        }
                        val value = JSONObject(RustPortal.start(dir.absolutePath))
                        val found = value.optString("origin")
                        require(validOrigin(found))
                        EmployeeProcess.origin = found
                        runOnUiThread { serviceOrigin = found }
                    } catch (_: Exception) { runOnUiThread { failure = "The company workspace could not start. Reopen the app or reinstall your company package." } }
                }
            }
        }
    }
    // JavaScript runs only bundled Rust-served pages under strict CSP/origin rules.
    @SuppressLint("SetJavaScriptEnabled")
    private fun protectedBrowser(local: String): WebView {
        require(validOrigin(local)); origin = local
        EmployeeProcess.view?.let { existing ->
            (existing.parent as? ViewGroup)?.removeView(existing)
            browser = existing; return existing
        }
        val view = WebView(applicationContext)
        browser = view; EmployeeProcess.view = view
        view.importantForAutofill = android.view.View.IMPORTANT_FOR_AUTOFILL_NO_EXCLUDE_DESCENDANTS
        WebView.setWebContentsDebuggingEnabled(false)
        view.settings.apply {
            javaScriptEnabled = true; domStorageEnabled = false
            cacheMode = WebSettings.LOAD_NO_CACHE
            allowFileAccess = false; allowContentAccess = true
            mixedContentMode = WebSettings.MIXED_CONTENT_NEVER_ALLOW
            javaScriptCanOpenWindowsAutomatically = false; setSupportMultipleWindows(false)
            mediaPlaybackRequiresUserGesture = true
        }
        CookieManager.getInstance().setAcceptThirdPartyCookies(view, false)
        view.webViewClient = object : WebViewClient() {
            override fun shouldOverrideUrlLoading(v: WebView, request: WebResourceRequest): Boolean = !sameOrigin(request.url, local)
            override fun onRenderProcessGone(v: WebView, detail: RenderProcessGoneDetail): Boolean {
                (v.parent as? ViewGroup)?.removeView(v)
                EmployeeProcess.view = null
                v.destroy()
                EmployeeProcess.activity.get()?.let { activity ->
                    activity.browser = null
                    AlertDialog.Builder(activity).setMessage("The employee screen closed unexpectedly. Reopen it and check saved records before repeating a change.").setCancelable(false).setPositiveButton("Reopen") { _, _ -> activity.recreate() }.show()
                }
                return true
            }
            override fun onPageFinished(v: WebView, url: String) {
                if (sameOrigin(Uri.parse(url), local)) v.evaluateJavascript("window.print=()=>window.HrisAndroid?.postMessage(JSON.stringify({type:'print'}));", null)
            }
        }
        view.webChromeClient = object : WebChromeClient() {
            override fun onShowFileChooser(v: WebView, callback: ValueCallback<Array<Uri>>, options: FileChooserParams): Boolean {
                val activity = EmployeeProcess.activity.get() ?: return false
                if (!sameOrigin(Uri.parse(v.url ?: ""), local) || options.mode != FileChooserParams.MODE_OPEN) return false
                activity.pendingPhoto?.onReceiveValue(null); activity.pendingPhoto = callback
                activity.photoPicker.launch("image/*"); return true
            }
            override fun onJsConfirm(v: WebView, url: String, text: String, result: JsResult): Boolean {
                val activity = EmployeeProcess.activity.get() ?: run { result.cancel(); return true }
                if (!sameOrigin(Uri.parse(url), local)) { result.cancel(); return true }
                AlertDialog.Builder(activity).setMessage(text.take(1000)).setPositiveButton("Continue") { _, _ -> result.confirm() }.setNegativeButton("Cancel") { _, _ -> result.cancel() }.setOnCancelListener { result.cancel() }.show(); return true
            }
        }
        if (WebViewFeature.isFeatureSupported(WebViewFeature.WEB_MESSAGE_LISTENER)) {
        WebViewCompat.addWebMessageListener(view, "HrisAndroid", setOf(local)) { v, message, source, main, _ ->
            if (!main || source.toString().trimEnd('/') != local || message.type != WebMessageCompat.TYPE_STRING) return@addWebMessageListener
            val activity = EmployeeProcess.activity.get() ?: return@addWebMessageListener
            try {
                val body = message.data ?: return@addWebMessageListener
                if (body.length > 2_000_000) return@addWebMessageListener
                val value = JSONObject(body)
                when (value.optString("type")) {
                    "print" -> if (sameOrigin(Uri.parse(v.url ?: ""), local)) {
                        (activity.getSystemService(PRINT_SERVICE) as PrintManager).print("HRIS payslip", v.createPrintDocumentAdapter("HRIS payslip"), null)
                    }
                    "csv" -> {
                        val name = value.optString("filename")
                        if (!Regex("payslip-\\d{4}-\\d{2}\\.csv").matches(name) || activity.exportBytes != null) return@addWebMessageListener
                        activity.exportBytes = value.getString("content").toByteArray(Charsets.UTF_8)
                        activity.exportPicker.launch(name)
                    }
                }
            } catch (_: Exception) { activity.message("This action could not be completed. Refresh and try again.") }
        }
        } else {
            view.settings.javaScriptEnabled = false
            view.loadData("<p>Update Android System WebView to open your company workspace safely.</p>", "text/html", "UTF-8")
            return view
        }
        view.loadUrl(local)
        return view
    }
    private fun notifications() {
        // Do not ask for an OS permission before a company push connection exists.
        message("Your in-app inbox is available. Background device alerts are not connected yet.")
    }
    private fun message(text: String) { if (!isFinishing) AlertDialog.Builder(this).setMessage(text).setPositiveButton("OK", null).show() }
    override fun onPause() { browser?.onPause(); super.onPause() }
    override fun onResume() { super.onResume(); browser?.onResume() }
    override fun onDestroy() {
        pendingPhoto?.onReceiveValue(null); pendingPhoto = null; exportBytes = null
        if (isFinishing) {
            EmployeeProcess.view = null; EmployeeProcess.origin = null
            browser?.let { (it.parent as? ViewGroup)?.removeView(it); it.stopLoading(); it.loadUrl("about:blank"); it.clearHistory(); it.clearCache(true); it.destroy() }
            CookieManager.getInstance().removeAllCookies(null); WebStorage.getInstance().deleteAllData()
            EmployeeProcess.worker.execute { RustPortal.stop() }
        } else (browser?.parent as? ViewGroup)?.removeView(browser)
        if (EmployeeProcess.activity.get() === this) EmployeeProcess.activity.clear()
        super.onDestroy()
    }
    companion object {
        internal fun validOrigin(value: String): Boolean = try {
            val uri = Uri.parse(value)
            uri.scheme == "http" && uri.host == "127.0.0.1" && uri.port in 1024..65535 && uri.path.orEmpty().isEmpty() && uri.query == null && uri.fragment == null && uri.userInfo == null
        } catch (_: Exception) { false }
        internal fun sameOrigin(uri: Uri, local: String): Boolean = validOrigin(local) && uri.scheme == "http" && uri.host == "127.0.0.1" && uri.port == Uri.parse(local).port && uri.userInfo == null
    }
}
