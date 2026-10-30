package io.github.tatselkrik.hrisemployee
import android.app.Notification
import android.content.Intent
import android.app.NotificationManager
import android.os.SystemClock
import android.webkit.CookieManager
import androidx.lifecycle.Lifecycle
import androidx.test.core.app.ActivityScenario
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import org.json.JSONObject
import java.io.File
import java.net.HttpURLConnection
import java.net.URI
import java.util.UUID
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference
@RunWith(AndroidJUnit4::class)
class DeviceAlertsTest {
 private val instrument=InstrumentationRegistry.getInstrumentation()
 private fun eval(s:ActivityScenario<MainActivity>,js:String):String {val value=AtomicReference<String>();val done=CountDownLatch(1);s.onActivity{it.browser?.evaluateJavascript(js){v->value.set(v);done.countDown()}?:done.countDown()};assertTrue(done.await(10,TimeUnit.SECONDS));return value.get()?:"null"}
 private fun await(s:ActivityScenario<MainActivity>,js:String){val until=SystemClock.elapsedRealtime()+90000;while(SystemClock.elapsedRealtime()<until){if(eval(s,js)=="true")return;SystemClock.sleep(250)};fail("Expected device-alert state was not reached: "+eval(s,"JSON.stringify({busy:state.busy,notice:document.getElementById('notice')?.textContent,loginNotice:document.getElementById('login-notice')?.textContent,formValid:document.getElementById('sign-in-form')?.checkValidity(),alerts:deviceAlerts.dialog?.querySelector('p')?.textContent})"))}
 private fun evalCurrent(js:String):String {
  val value=AtomicReference<String>();val done=CountDownLatch(1)
  instrument.runOnMainSync { EmployeeProcess.activity.get()?.browser?.evaluateJavascript(js){v->value.set(v);done.countDown()}?:done.countDown() }
  assertTrue("Current activity WebView response",done.await(10,TimeUnit.SECONDS));return value.get()?:"null"
 }
 private fun awaitCurrent(js:String){val until=SystemClock.elapsedRealtime()+90000;while(SystemClock.elapsedRealtime()<until){if(evalCurrent(js)=="true")return;SystemClock.sleep(250)};fail("Current notification-opened activity did not reach expected own state")}
 @Test fun own_opt_in_background_delivery_open_and_sign_out(){
  assertTrue(BuildConfig.PUSH_BACKEND_ENABLED)
  val context=instrument.targetContext;DeviceAlertPreferences.disable(context);File(context.filesDir,"qa-push-received").delete();val file=File(context.filesDir,"qa-credentials.json");assertTrue(file.exists());val credentials=JSONObject(file.readText());val email=credentials.getString("email");assertTrue(email.startsWith("qa-gov042-acceptance069-")&&email.endsWith("@example.invalid"))
  val realTray=InstrumentationRegistry.getArguments().getString("qa_real_tray")=="true"
  val s=ActivityScenario.launch(MainActivity::class.java)
  try {
   await(s,"typeof state!=='undefined'&&!state.busy&&!document.getElementById('sign-in-page').hidden")
   eval(s,"document.getElementById('email').value="+JSONObject.quote(email)+";document.getElementById('password').value="+JSONObject.quote(credentials.getString("password"))+";document.getElementById('sign-in-form').requestSubmit();true")
   await(s,"!!state.me&&!state.busy")
   eval(s,"window.deviceAlertsOpen();true");await(s,"!!deviceAlerts.dialog&&Array.from(deviceAlerts.dialog.querySelectorAll('button')).some(b=>b.textContent==='Enable alerts')")
   eval(s,"Array.from(deviceAlerts.dialog.querySelectorAll('button')).find(b=>b.textContent==='Enable alerts').click();true")
   await(s,"!!deviceAlerts.token&&!!deviceAlerts.dialog&&Array.from(deviceAlerts.dialog.querySelectorAll('button')).some(b=>b.textContent==='Disable alerts')")
   assertTrue(DeviceAlertPreferences.enabled(context))
   assertEquals("true",eval(s,"document.documentElement.scrollWidth<=innerWidth+1&&deviceAlerts.dialog.getBoundingClientRect().left>=0&&deviceAlerts.dialog.getBoundingClientRect().right<=innerWidth"))
   val screenshot=instrument.uiAutomation.takeScreenshot();assertNotNull(screenshot);File(context.filesDir,"qa-device-alerts.png").outputStream().use{screenshot!!.compress(android.graphics.Bitmap.CompressFormat.PNG,100,it)};screenshot!!.recycle()
   val input=JSONObject(JSONObject("{\"v\":"+eval(s,"JSON.stringify({token:deviceAlerts.token,csrf:state.csrf})")+"}").getString("v"));val local=AtomicReference<String>();val cookie=AtomicReference<String>();s.onActivity{local.set(it.origin);cookie.set(CookieManager.getInstance().getCookie(it.origin!!))}
   eval(s,"deviceAlerts.dialog.close();true");s.moveToState(Lifecycle.State.CREATED)
   val body=JSONObject().put("token",input.getString("token")).put("request_id",UUID.randomUUID().toString()).toString()
   val connection=URI(local.get()+"/api/device-alerts/test").toURL().openConnection()as HttpURLConnection
   connection.requestMethod="POST";connection.doOutput=true;connection.connectTimeout=20000;connection.readTimeout=60000
   connection.setRequestProperty("Origin",local.get());connection.setRequestProperty("Cookie",cookie.get());connection.setRequestProperty("X-CSRF-Token",input.getString("csrf"));connection.setRequestProperty("Content-Type","application/json")
   connection.outputStream.use{it.write(body.toByteArray())};assertEquals("Protected own test must succeed",200,connection.responseCode)
   val result=JSONObject(connection.inputStream.bufferedReader().use{it.readText()});assertEquals("sent",result.getString("state"));connection.disconnect()
   val manager=context.getSystemService(NotificationManager::class.java);val until=SystemClock.elapsedRealtime()+60000
   while(SystemClock.elapsedRealtime()<until&&manager.activeNotifications.none{it.id==1042})SystemClock.sleep(250)
   val received=manager.activeNotifications.firstOrNull{it.id==1042};assertNotNull("Actual background FCM reception",received)
   val notification=received!!.notification;assertEquals("Workforce Employee",notification.extras.getString(Notification.EXTRA_TITLE));assertEquals("You have an HRIS update. Open the app to review your inbox.",notification.extras.getString(Notification.EXTRA_TEXT));assertEquals(Notification.VISIBILITY_PRIVATE,notification.visibility)
   File(context.filesDir,"qa-push-received").writeText("received")
   if(realTray) {
    assertTrue("Open only the owned emulator's notification shade",instrument.uiAutomation.performGlobalAction(android.accessibilityservice.AccessibilityService.GLOBAL_ACTION_NOTIFICATIONS))
    fun findAlert(node:android.view.accessibility.AccessibilityNodeInfo):android.view.accessibility.AccessibilityNodeInfo? {
     if(node.packageName?.toString()=="com.android.systemui"&&node.text?.toString()=="You have an HRIS update. Open the app to review your inbox.")return node
     for(i in 0 until node.childCount){val child=node.getChild(i)?:continue;findAlert(child)?.let{return it}}
     return null
    }
    var alert:android.view.accessibility.AccessibilityNodeInfo?=null
    val deadline=SystemClock.elapsedRealtime()+30000
    while(alert==null&&SystemClock.elapsedRealtime()<deadline){alert=instrument.uiAutomation.rootInActiveWindow?.let{findAlert(it)};if(alert==null)SystemClock.sleep(200)}
    assertNotNull("Actual generic HRIS system notification must be shown",alert)
    alert!!.performAction(android.view.accessibility.AccessibilityNodeInfo.AccessibilityAction.ACTION_SHOW_ON_SCREEN.id);SystemClock.sleep(400)
    assertTrue("Own notification is visible in the actual shade",alert!!.isVisibleToUser)
    File(context.filesDir,"qa-tray-progress.json").writeText("{\"stage\":\"actual-notification-visible\"}")
    instrument.uiAutomation.takeScreenshot()?.let{image->File(context.filesDir,"qa-system-tray.png").outputStream().use{image.compress(android.graphics.Bitmap.CompressFormat.PNG,100,it)};image.recycle()}
    var clicked=false
    for(i in 0..7){val target=alert?:break;if(target.performAction(android.view.accessibility.AccessibilityNodeInfo.ACTION_CLICK)){clicked=true;break};alert=target.parent}
    assertTrue("Tap the actual system notification",clicked)
    awaitCurrent("state.page==='notifications'&&!state.busy&&!!state.me")
    File(context.filesDir,"qa-tray-progress.json").writeText("{\"stage\":\"actual-tap-own-inbox-verified\"}")
    File(context.filesDir,"qa-tray-tap-assertions").writeText("actual-system-tray-to-own-inbox-passed")
   } else {
    s.moveToState(Lifecycle.State.RESUMED);s.onActivity{instrument.callActivityOnNewIntent(it,Intent(it.intent).putExtra("hris_open_inbox",true))};await(s,"state.page==='notifications'&&!state.busy")
   }
   if(realTray){evalCurrent("signOut();true");awaitCurrent("!state.me&&!state.busy")}else{eval(s,"signOut();true");await(s,"!state.me&&!state.busy")}
   assertFalse(DeviceAlertPreferences.enabled(context));assertTrue(manager.activeNotifications.none{it.id==1042});File(context.filesDir,"qa-push-assertions").writeText("passed")
   if(realTray)File(context.filesDir,"qa-tray-progress.json").writeText("{\"stage\":\"actual-tap-own-inbox-and-signout-complete\"}")
  } finally {
   // Real onNewIntent replaces the launch intent; observe/finish the actual Activity
   // without depending on ActivityScenario's instrumentation-only launch cookie.
   if(realTray){instrument.runOnMainSync{EmployeeProcess.activity.get()?.finish()};instrument.waitForIdleSync()}else s.close()
  }
 }
 @Test fun device_alert_dialog_visual_only(){
  assertTrue(BuildConfig.PUSH_BACKEND_ENABLED)
  val context=instrument.targetContext;DeviceAlertPreferences.disable(context);File(context.filesDir,"qa-push-received").delete();val file=File(context.filesDir,"qa-credentials.json");assertTrue(file.exists());val credentials=JSONObject(file.readText());val email=credentials.getString("email");assertTrue(email.startsWith("qa-gov042-acceptance069-")&&email.endsWith("@example.invalid"))
  ActivityScenario.launch(MainActivity::class.java).use{s->
   await(s,"typeof state!=='undefined'&&!state.busy&&!document.getElementById('sign-in-page').hidden")
   eval(s,"document.getElementById('email').value="+JSONObject.quote(email)+";document.getElementById('password').value="+JSONObject.quote(credentials.getString("password"))+";document.getElementById('sign-in-form').requestSubmit();true")
   await(s,"!!state.me&&!state.busy")
   eval(s,"window.deviceAlertsOpen();true");await(s,"!!deviceAlerts.dialog&&Array.from(deviceAlerts.dialog.querySelectorAll('button')).some(b=>b.textContent==='Enable alerts')")
   eval(s,"Array.from(deviceAlerts.dialog.querySelectorAll('button')).find(b=>b.textContent==='Enable alerts').click();true")
   await(s,"!!deviceAlerts.token&&!!deviceAlerts.dialog&&Array.from(deviceAlerts.dialog.querySelectorAll('button')).some(b=>b.textContent==='Disable alerts')")
   assertTrue(DeviceAlertPreferences.enabled(context))
   assertEquals("true",eval(s,"document.documentElement.scrollWidth<=innerWidth+1&&deviceAlerts.dialog.getBoundingClientRect().left>=0&&deviceAlerts.dialog.getBoundingClientRect().right<=innerWidth"))

   instrument.waitForIdleSync();SystemClock.sleep(800)
   val raw=eval(s,"JSON.stringify({innerHeight,innerWidth,visualHeight:visualViewport.height,height:deviceAlerts.dialog.getBoundingClientRect().height,clientHeight:deviceAlerts.dialog.clientHeight,scrollHeight:deviceAlerts.dialog.scrollHeight,maxHeight:getComputedStyle(deviceAlerts.dialog).maxHeight,open:deviceAlerts.dialog.open,lastBottom:deviceAlerts.dialog.lastElementChild.getBoundingClientRect().bottom,bottom:deviceAlerts.dialog.getBoundingClientRect().bottom})")
   val layout=JSONObject(JSONObject("{\"v\":"+raw+"}").getString("v"));File(context.filesDir,"qa-alert-layout.json").writeText(layout.toString())
   val image=instrument.uiAutomation.takeScreenshot();assertNotNull(image);File(context.filesDir,"qa-device-alerts.png").outputStream().use{image!!.compress(android.graphics.Bitmap.CompressFormat.PNG,100,it)};image!!.recycle()
   assertTrue("Alert content must fit after drawing: "+layout.toString(),layout.getDouble("height")>180&&layout.getDouble("lastBottom")<=layout.getDouble("bottom")+1)
   eval(s,"deviceAlerts.dialog.close();signOut();true");await(s,"!state.me&&!state.busy");assertFalse(DeviceAlertPreferences.enabled(context))
  }
 }
 @Test fun prepare_signed_release_alert_grant(){
  assertTrue(BuildConfig.PUSH_BACKEND_ENABLED)
  val context=instrument.targetContext;DeviceAlertPreferences.disable(context);File(context.filesDir,"qa-push-received").delete();val file=File(context.filesDir,"qa-credentials.json");assertTrue(file.exists());val credentials=JSONObject(file.readText());val email=credentials.getString("administrator_email");assertTrue(email.startsWith("qa-gov042-acceptance069-")&&email.endsWith("@example.invalid"))
  ActivityScenario.launch(MainActivity::class.java).use{s->
   await(s,"typeof state!=='undefined'&&!state.busy&&!document.getElementById('sign-in-page').hidden")
   eval(s,"document.getElementById('email').value="+JSONObject.quote(email)+";document.getElementById('password').value="+JSONObject.quote(credentials.getString("password"))+";document.getElementById('sign-in-form').requestSubmit();true")
   await(s,"!!state.me&&!state.busy")
   eval(s,"window.deviceAlertsOpen();true");await(s,"!!deviceAlerts.dialog&&Array.from(deviceAlerts.dialog.querySelectorAll('button')).some(b=>b.textContent==='Enable alerts')")
   eval(s,"Array.from(deviceAlerts.dialog.querySelectorAll('button')).find(b=>b.textContent==='Enable alerts').click();true")
   await(s,"!!deviceAlerts.token&&!!deviceAlerts.dialog&&Array.from(deviceAlerts.dialog.querySelectorAll('button')).some(b=>b.textContent==='Disable alerts')")
   assertTrue(DeviceAlertPreferences.enabled(context))
   // Closing revokes the local Auth session; only the generic device grant survives.
  }
 }
}
