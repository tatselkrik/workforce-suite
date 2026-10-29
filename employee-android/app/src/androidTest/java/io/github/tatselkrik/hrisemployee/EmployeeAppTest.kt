package io.github.tatselkrik.hrisemployee

import android.content.Intent
import android.content.IntentFilter
import android.app.Activity
import android.app.Instrumentation
import android.os.Bundle
import android.os.CancellationSignal
import android.os.ParcelFileDescriptor
import android.print.PrintAttributes
import android.print.PrintDocumentAdapter
import android.print.PrintDocumentInfo
import android.print.PageRange
import android.os.SystemClock
import android.view.KeyEvent
import androidx.test.core.app.ActivityScenario
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.json.JSONObject
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File
import java.net.Socket
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference

@RunWith(AndroidJUnit4::class)
class EmployeeAppTest {
    private val instrument = InstrumentationRegistry.getInstrumentation()
    private fun evaluate(scenario: ActivityScenario<MainActivity>, expression: String): String {
        val result = AtomicReference<String>(); val done = CountDownLatch(1)
        scenario.onActivity { activity ->
            val web = activity.browser
            if (web == null) { result.set("null"); done.countDown() }
            else web.evaluateJavascript(expression) { result.set(it); done.countDown() }
        }
        assertTrue("WebView evaluation timed out", done.await(10, TimeUnit.SECONDS)); return result.get() ?: "null"
    }
    private fun await(scenario: ActivityScenario<MainActivity>, expression: String, seconds: Int = 60) {
        val until = SystemClock.elapsedRealtime() + seconds * 1000
        while (SystemClock.elapsedRealtime() < until) { if (evaluate(scenario, expression) == "true") return; SystemClock.sleep(250) }
        capture("qa-failure.png")
        val diagnosis = evaluate(scenario, "JSON.stringify({focused:document.activeElement?.id,busy:typeof state!=='undefined'&&state.busy,notice:document.getElementById('notice')?.textContent,loginNotice:document.getElementById('login-notice')?.textContent,valid:document.getElementById('sign-in-form')?.checkValidity()})")
        fail("Expected employee UI state was not reached: "+diagnosis)
    }
    private fun capture(name: String) {
        val image = instrument.uiAutomation.takeScreenshot() ?: error("Screenshot unavailable")
        File(instrument.targetContext.filesDir, name).outputStream().use { image.compress(android.graphics.Bitmap.CompressFormat.PNG, 100, it) }
        image.recycle()
    }
    @Test fun company_shell_and_loopback_security() {
        ActivityScenario.launch(MainActivity::class.java).use { scenario ->
            await(scenario, "document.readyState==='complete' && typeof state!=='undefined' && !state.busy && !!document.getElementById('sign-in-form') && !document.querySelector('#sign-in-form button').disabled")
            val origin = AtomicReference<String>(); scenario.onActivity { activity ->
                assertFalse(activity.browser!!.settings.domStorageEnabled)
                assertFalse(activity.browser!!.settings.allowFileAccess)
                origin.set(activity.origin!!)
            }
            assertTrue(MainActivity.validOrigin(origin.get())); assertFalse(MainActivity.validOrigin("http://0.0.0.0:3030")); assertFalse(MainActivity.validOrigin("https://foreign.invalid"))
            val uri = android.net.Uri.parse(origin.get())
            Socket("127.0.0.1", uri.port).use { socket ->
                socket.soTimeout = 10000
                socket.getOutputStream().write("GET /api/me HTTP/1.1\r\nHost: foreign.invalid\r\nConnection: close\r\n\r\n".toByteArray())
                val first = socket.getInputStream().bufferedReader().readLine(); assertTrue(first.contains("403") || first.contains("400"))
            }
            assertEquals("true", evaluate(scenario, "document.documentElement.scrollWidth <= innerWidth+1"))
            assertEquals("true", evaluate(scenario, "document.getElementById('login-company').textContent.length>0"))
            capture("qa-signin.png")
            evaluate(scenario, "location.href='https://foreign.invalid';true")
            SystemClock.sleep(500)
            assertEquals("true", evaluate(scenario, "location.origin==='"+origin.get()+"'"))
        }
    }
    @Test fun existing_employee_signin_own_records_and_payslip() {
        val file = File(instrument.targetContext.filesDir, "qa-credentials.json")
        assertTrue("Managed synthetic credentials must be provisioned by the guarded harness", file.exists())
        val values = JSONObject(file.readText()); val email = values.getString("email"); val password = values.getString("password"); val code = values.getString("employee_code")
        assertTrue(email.startsWith("qa-gov042-acceptance069-") && email.endsWith("@example.invalid")); assertTrue(code.startsWith("QA-AT069-HIRE-"))
        ActivityScenario.launch(MainActivity::class.java).use { scenario ->
            await(scenario, "document.readyState==='complete' && typeof state!=='undefined' && !state.busy && !!document.getElementById('sign-in-form') && !document.querySelector('#sign-in-form button').disabled")
            evaluate(scenario, "document.getElementById('email').value="+JSONObject.quote(email)+";document.getElementById('password').value="+JSONObject.quote(password)+";document.getElementById('password').focus();true")
            scenario.onActivity { it.browser!!.requestFocus() }
            instrument.sendKeySync(KeyEvent(KeyEvent.ACTION_DOWN, KeyEvent.KEYCODE_ENTER)); instrument.sendKeySync(KeyEvent(KeyEvent.ACTION_UP, KeyEvent.KEYCODE_ENTER))
            await(scenario, "!document.getElementById('workspace').hidden", 100)
            assertEquals("true", evaluate(scenario, "state.me.employment.employee_code==="+JSONObject.quote(code)))
            assertEquals("true", evaluate(scenario, "document.getElementById('password').value===''"))
            assertEquals("true", evaluate(scenario, "!document.querySelector('input[name=employee_code]')"))
            evaluate(scenario, "document.querySelector('[data-page=pay]').click();true")
            await(scenario, "document.querySelectorAll('#payslip-list button').length>0")
            evaluate(scenario, "document.querySelector('#payslip-list button').click();true")
            await(scenario, "!document.getElementById('payslip-detail').hidden")
            assertEquals("true", evaluate(scenario, "state.slip.employee.employee_code==="+JSONObject.quote(code)+" && state.slip.totals.net===2654245"))
            assertEquals("true", evaluate(scenario, "typeof HrisAndroid.postMessage==='function' && typeof window.print==='function'"))
            capture("qa-payslip.png")
            evaluate(scenario, "window.print();true")
            val printDeadline=SystemClock.elapsedRealtime()+15000
            while(instrument.uiAutomation.rootInActiveWindow?.packageName?.toString()!="com.android.printspooler" && SystemClock.elapsedRealtime()<printDeadline) SystemClock.sleep(100)
            assertEquals("Android print dialog", "com.android.printspooler",instrument.uiAutomation.rootInActiveWindow?.packageName?.toString())
            capture("qa-print.png")
            instrument.sendKeyDownUpSync(KeyEvent.KEYCODE_BACK)
            val returnDeadline=SystemClock.elapsedRealtime()+5000
            while(instrument.uiAutomation.rootInActiveWindow?.packageName?.toString()!=instrument.targetContext.packageName && SystemClock.elapsedRealtime()<returnDeadline)SystemClock.sleep(100)
            assertEquals("Returned from printing",instrument.targetContext.packageName,instrument.uiAutomation.rootInActiveWindow?.packageName?.toString())
            await(scenario,"location.origin.startsWith('http://127.0.0.1:') && !state.busy")
            val csvMonitor = instrument.addMonitor(IntentFilter(Intent.ACTION_CREATE_DOCUMENT).apply { addCategory(Intent.CATEGORY_OPENABLE); addDataType("text/csv") }, Instrumentation.ActivityResult(Activity.RESULT_CANCELED, null), true)
            try {
                evaluate(scenario, "Array.from(document.querySelectorAll('#payslip-detail button')).find(b=>b.textContent==='Download CSV').click();true")
                val deadline = SystemClock.elapsedRealtime()+5000
                while(csvMonitor.hits==0 && SystemClock.elapsedRealtime()<deadline) SystemClock.sleep(100)
                if(csvMonitor.hits==0) capture("qa-failure.png")
                assertTrue("CSV must use the Android document chooser", csvMonitor.hits>0)
            } finally { instrument.removeMonitor(csvMonitor) }
            for (page in listOf("attendance", "leave", "services", "profile")) {
                evaluate(scenario, "document.querySelector('[data-page="+page+"]').click();true")
                await(scenario, "state.page==='"+page+"' && !state.busy")
                if(evaluate(scenario, "document.documentElement.scrollWidth<=innerWidth+1")!="true") capture("qa-failure.png")
                assertEquals("Mobile layout: "+page, "true", evaluate(scenario, "document.documentElement.scrollWidth<=innerWidth+1"))
            }
            val photoMonitor = instrument.addMonitor(IntentFilter(Intent.ACTION_GET_CONTENT).apply { addCategory(Intent.CATEGORY_OPENABLE); addDataType("image/*") }, Instrumentation.ActivityResult(Activity.RESULT_CANCELED, null), true)
            try {
                scenario.onActivity { it.browser!!.requestFocus() }
                evaluate(scenario, "document.getElementById('photo-file').focus();true")
                instrument.sendKeyDownUpSync(KeyEvent.KEYCODE_ENTER)
                val deadline = SystemClock.elapsedRealtime()+5000
                while(photoMonitor.hits==0 && SystemClock.elapsedRealtime()<deadline) SystemClock.sleep(100)
                if(photoMonitor.hits==0) capture("qa-failure.png")
                assertTrue("Photo uses an Android user-selected image", photoMonitor.hits>0)
            } finally { instrument.removeMonitor(photoMonitor) }
            // Retained process state must not lose a contact draft on Activity recreation.
            evaluate(scenario, "document.getElementById('phone').value='09170000690';document.getElementById('phone').dispatchEvent(new Event('input',{bubbles:true}));true")
            assertEquals("true", evaluate(scenario, "Boolean(state.dirty)"))
            scenario.recreate()
            await(scenario, "typeof state!=='undefined' && state.me!==null")
            assertEquals("true", evaluate(scenario, "state.dirty && document.getElementById('phone').value==='09170000690'"))
            evaluate(scenario, "document.getElementById('contacts-discard').click();true")
            await(scenario, "!state.dirty")
            evaluate(scenario, "document.getElementById('sign-out').click();true")
            await(scenario, "!document.getElementById('sign-in-page').hidden")
            assertEquals("true", evaluate(scenario, "state.me===null && state.csrf===null && document.getElementById('phone').value===''"))
            val admin = values.getString("administrator_email")
            assertTrue(admin.startsWith("qa-gov042-acceptance069-") && admin.endsWith("@example.invalid"))
            evaluate(scenario, "document.getElementById('email').value="+JSONObject.quote(admin)+";document.getElementById('password').value="+JSONObject.quote(password)+";document.getElementById('sign-in-form').requestSubmit();true")
            await(scenario, "!document.getElementById('workspace').hidden", 100)
            assertEquals("true", evaluate(scenario, "state.me.employment.employee_code==="+JSONObject.quote(values.getString("administrator_code"))+" && state.payslips.length===0"))
            val slip = values.getString("slip_id")
            evaluate(scenario, "window.qaForeignDenied=false;fetch('/api/payslips/"+slip+"').then(r=>window.qaForeignDenied=r.status===404);true")
            await(scenario, "window.qaForeignDenied===true")
            evaluate(scenario, "document.getElementById('sign-out').click();true")
            await(scenario, "!document.getElementById('sign-in-page').hidden")
        }
    }
}
