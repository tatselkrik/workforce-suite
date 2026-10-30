package io.github.tatselkrik.hrisemployee

import android.os.SystemClock
import android.webkit.WebView
import androidx.test.core.app.ActivityScenario
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.json.JSONObject
import org.json.JSONTokener
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference

/** Real owned WebView renders with test-only fictional state; no backend sign-in or writes. */
@RunWith(AndroidJUnit4::class)
class DesignPreviewTest {
    private val instrument = InstrumentationRegistry.getInstrumentation()
    private fun evaluate(scenario: ActivityScenario<MainActivity>, expression: String): String {
        val result = AtomicReference<String>(); val done = CountDownLatch(1)
        scenario.onActivity { a -> a.browser?.evaluateJavascript(expression) { v -> result.set(v); done.countDown() } ?: done.countDown() }
        assertTrue("WebView response",done.await(10,TimeUnit.SECONDS));return result.get() ?: "null"
    }
    private fun await(scenario: ActivityScenario<MainActivity>, expression: String) {
        val until = SystemClock.elapsedRealtime()+45000
        while(SystemClock.elapsedRealtime()<until){if(evaluate(scenario,expression)=="true")return;SystemClock.sleep(150)}
        fail("Design page state unavailable: "+expression.take(180))
    }
    private fun painted(scenario: ActivityScenario<MainActivity>) {
        evaluate(scenario,"window.__designPaint=false;requestAnimationFrame(()=>requestAnimationFrame(()=>window.__designPaint=true));true")
        await(scenario,"window.__designPaint===true")
        val ready=CountDownLatch(1)
        scenario.onActivity { it.browser!!.postVisualStateCallback(1L,object : WebView.VisualStateCallback() {
            override fun onComplete(id: Long) { ready.countDown() }
        }) }
        assertTrue("Composited WebView state",ready.await(10,TimeUnit.SECONDS))
        val drawn=CountDownLatch(1)
        scenario.onActivity { a -> a.browser!!.apply { invalidate();postOnAnimation { postOnAnimation { drawn.countDown() } } } }
        assertTrue("Native frames after visual state",drawn.await(10,TimeUnit.SECONDS))
        instrument.waitForIdleSync();SystemClock.sleep(750)
    }
    private fun capture(name: String) {
        val image=instrument.uiAutomation.takeScreenshot() ?: error("Owned emulator screenshot unavailable")
        File(instrument.targetContext.filesDir,"design-$name.png").outputStream().use{image.compress(android.graphics.Bitmap.CompressFormat.PNG,100,it)};image.recycle()
    }
    @Test fun fictional_design_layouts_and_draft_guards() {
        val args=InstrumentationRegistry.getArguments();val tag=args.getString("design_case") ?: "phone-light"
        require(tag.matches(Regex("[a-z0-9-]{1,40}")))
        val compact=tag.startsWith("compact");val tablet=tag.startsWith("tablet")
        val proof=JSONObject().put("fictional_only",true).put("backend_authenticated",false)
        ActivityScenario.launch(MainActivity::class.java).use { scenario ->
            await(scenario,"document.readyState==='complete' && typeof state!=='undefined' && !state.busy")
            if(compact)scenario.onActivity { it.browser!!.settings.textZoom=130 }
            painted(scenario)
            val mode=evaluate(scenario,"matchMedia('(prefers-color-scheme:dark)').matches")
            assertEquals("Actual system appearance",tag.contains("dark").toString(),mode)
            val marker=evaluate(scenario,"getComputedStyle(document.documentElement).getPropertyValue('--bg').trim()")
            assertEquals(if(tag.contains("dark")) "\"#131d28\"" else "\"#f6f9fa\"",marker)
            evaluate(scenario,"document.getElementById('login-company').textContent='HRIS Company';document.getElementById('login-logo').hidden=true;document.getElementById('login-logo').removeAttribute('src');document.getElementById('login-initials').hidden=false;document.getElementById('login-initials').textContent='HC';document.getElementById('login-logo').parentElement.classList.remove('has-image');true")
            painted(scenario);capture("$tag-signin")
            val fixture=instrument.context.assets.open("design-fixture.js").bufferedReader().use{it.readText()}
            val seeded=JSONObject(JSONTokener(evaluate(scenario,fixture)).nextValue().toString());assertTrue(seeded.toString(),seeded.getBoolean("ok"))
            evaluate(scenario,"window.__designIconsLoaded=0;window.__designIconsFailed=false;['home','attendance','leave','pay','services'].forEach(n=>{const i=new Image();i.onload=()=>window.__designIconsLoaded++;i.onerror=()=>window.__designIconsFailed=true;i.src='/android-icons/'+n+'.svg';});true")
            await(scenario,"window.__designIconsLoaded===5 || window.__designIconsFailed")
            assertEquals("All five bundled icons load under the actual CSP","5",evaluate(scenario,"window.__designIconsLoaded"))
            painted(scenario)
            val pages=if(tablet)listOf("home","profile")else if(compact)listOf("leave","profile")else listOf("home","attendance","leave","pay","services","profile","notifications")
            val metrics=JSONObject()
            for(page in pages){
                evaluate(scenario,"navigate('${page}');true");await(scenario,"state.page==='${page}' && !state.busy && !document.getElementById('page-${page}').hidden");painted(scenario)
                val raw=evaluate(scenario,"JSON.stringify({width:innerWidth,height:innerHeight,scroll:document.documentElement.scrollWidth,dark:matchMedia('(prefers-color-scheme:dark)').matches,mobileWidth:matchMedia('(max-width:760px)').matches,shortHeight:matchMedia('(max-height:500px)').matches,brand:getComputedStyle(document.querySelector('.company-brand .brand-mark')).color,nav:getComputedStyle(document.getElementById('navigation')).position,mask:getComputedStyle(document.querySelector('[data-page=home]'),'::before').maskImage})")
                val m=JSONObject(JSONTokener(raw).nextValue().toString());assertTrue("Horizontal overflow $tag/$page: $m",m.getInt("scroll")<=m.getInt("width")+1)
                assertTrue("Local navigation icon missing",m.getString("mask").startsWith("url("));metrics.put(page,m);capture("$tag-$page")
            }
            if(!compact && !tablet){
                evaluate(scenario,"navigate('pay');showPayslip(window.__designSlip);true");painted(scenario);capture("$tag-payslip")
                evaluate(scenario,"deviceAlertsOpen();true");await(scenario,"!!document.querySelector('.device-alert-dialog[open] .primary')");painted(scenario)
                val raw=evaluate(scenario,"JSON.stringify((()=>{const d=document.querySelector('.device-alert-dialog'),r=d.getBoundingClientRect();return{width:innerWidth,height:innerHeight,left:r.left,right:r.right,top:r.top,bottom:r.bottom,scroll:d.scrollHeight,visible:d.clientHeight}})())")
                val d=JSONObject(JSONTokener(raw).nextValue().toString());assertTrue("Alert dialog fits horizontally: $d",d.getDouble("left")>=0 && d.getDouble("right")<=d.getDouble("width")+1)
                assertTrue("Alert dialog fits vertically: $d",d.getDouble("top")>=0 && d.getDouble("bottom")<=d.getDouble("height")+1);assertTrue(d.getInt("visible")>100)
                capture("$tag-alerts");evaluate(scenario,"document.querySelector('.device-alert-dialog').close();true")
                proof.put("alert_dialog",d)
            }
            evaluate(scenario,"navigate('profile');state.dirty=true;navigate('home');true");assertEquals("Draft navigation remains blocked","\"profile\"",evaluate(scenario,"state.page"));evaluate(scenario,"state.dirty=false;true")
            if(tag=="phone-light") {
                val retained=AtomicReference<WebView>();scenario.onActivity { retained.set(it.browser) }
                evaluate(scenario,"document.getElementById('phone').value='fictional-unsaved';state.dirty=true;true")
                instrument.uiAutomation.executeShellCommand("cmd uimode night yes").close()
                await(scenario,"matchMedia('(prefers-color-scheme:dark)').matches")
                painted(scenario)
                scenario.onActivity { assertSame("Appearance preserves the live WebView",retained.get(),it.browser) }
                assertEquals("Appearance preserves unsaved text","\"fictional-unsaved\"",evaluate(scenario,"document.getElementById('phone').value"))
                assertEquals("Appearance preserves draft guard","true",evaluate(scenario,"state.dirty"))
                instrument.uiAutomation.executeShellCommand("cmd uimode night no").close()
                await(scenario,"!matchMedia('(prefers-color-scheme:dark)').matches")
                evaluate(scenario,"state.dirty=false;true");proof.put("live_theme_switch_preserves_draft",true)
            }
            assertEquals("No fictional preview can post data","0",evaluate(scenario,"window.__designWrites"))
            proof.put("metrics",metrics).put("draft_guard",true).put("write_attempts",0).put("bundled_icons_loaded",5).put("dark",mode=="true")
        }
        File(instrument.targetContext.filesDir,"design-$tag.json").writeText(proof.toString(2))
    }
}
