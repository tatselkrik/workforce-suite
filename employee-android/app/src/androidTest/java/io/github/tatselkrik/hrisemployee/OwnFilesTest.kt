package io.github.tatselkrik.hrisemployee

import android.accessibilityservice.AccessibilityServiceInfo
import android.content.ContentValues
import android.graphics.Bitmap
import android.graphics.Canvas
import android.graphics.Color
import android.graphics.Paint
import android.os.Bundle
import android.os.SystemClock
import android.provider.MediaStore
import android.view.KeyEvent
import android.view.accessibility.AccessibilityNodeInfo
import androidx.test.core.app.ActivityScenario
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import org.json.JSONArray
import org.json.JSONObject
import org.json.JSONTokener
import org.junit.Assert.*
import org.junit.Test
import org.junit.runner.RunWith
import java.io.File
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference

/** Actual Android chooser/upload/save checks, exclusively on the managed synthetic emulator. */
@RunWith(AndroidJUnit4::class)
class OwnFilesTest {
    private val instrument = InstrumentationRegistry.getInstrumentation()
    private val context get() = instrument.targetContext
    private val proof = JSONObject()
    private fun stage(value: String) {
        proof.put("stage", value)
        File(context.filesDir,"qa-own-files-proof.json").writeText(proof.toString(2))
    }
    private fun evaluate(scenario: ActivityScenario<MainActivity>, expression: String): String {
        val result=AtomicReference<String>(); val done=CountDownLatch(1)
        scenario.onActivity { it.browser!!.evaluateJavascript(expression) { value -> result.set(value);done.countDown() } }
        assertTrue("WebView response",done.await(10,TimeUnit.SECONDS));return result.get()
    }
    private fun await(scenario: ActivityScenario<MainActivity>, expression: String, seconds: Int=60) {
        val deadline=SystemClock.elapsedRealtime()+seconds*1000
        while(SystemClock.elapsedRealtime()<deadline) { if(evaluate(scenario,expression)=="true")return;SystemClock.sleep(250) }
        dump("web-state-timeout"); fail("Expected own-file page state: "+expression.take(220))
    }
    private fun find(predicate: (AccessibilityNodeInfo)->Boolean): AccessibilityNodeInfo? {
        val roots=instrument.uiAutomation.windows.mapNotNull { it.root }.ifEmpty { listOfNotNull(instrument.uiAutomation.rootInActiveWindow) }
        fun walk(node: AccessibilityNodeInfo): AccessibilityNodeInfo? {
            if(predicate(node))return node
            for(i in 0 until node.childCount) { val child=node.getChild(i)?:continue;val found=walk(child);if(found!=null)return found }
            return null
        }
        return roots.firstNotNullOfOrNull { walk(it) }
    }
    private fun node(label: String, seconds: Int=30, predicate: (AccessibilityNodeInfo)->Boolean): AccessibilityNodeInfo {
        val deadline=SystemClock.elapsedRealtime()+seconds*1000
        while(SystemClock.elapsedRealtime()<deadline) { find(predicate)?.let{return it};SystemClock.sleep(150) }
        dump(label);error("Android chooser control unavailable: "+label)
    }
    private fun click(label: String, predicate: (AccessibilityNodeInfo)->Boolean) {
        var n: AccessibilityNodeInfo?=node(label,predicate=predicate)
        repeat(6) { if(n?.isEnabled==true && n?.performAction(AccessibilityNodeInfo.ACTION_CLICK)==true)return;n=n?.parent }
        dump(label);error("Android control could not be clicked: "+label)
    }
    private fun capture(name: String) {
        val image=instrument.uiAutomation.takeScreenshot()?:error("Owned emulator screenshot unavailable")
        File(context.filesDir,name).outputStream().use { image.compress(Bitmap.CompressFormat.PNG,100,it) };image.recycle()
    }
    private fun dump(label: String) {
        val items=JSONArray()
        fun walk(n:AccessibilityNodeInfo,depth:Int) {
            if(items.length()>500)return
            items.put(JSONObject().put("depth",depth).put("package",n.packageName).put("id",n.viewIdResourceName).put("text",n.text).put("description",n.contentDescription).put("class",n.className).put("clickable",n.isClickable).put("enabled",n.isEnabled))
            for(i in 0 until n.childCount)n.getChild(i)?.let{walk(it,depth+1)}
        }
        instrument.uiAutomation.windows.mapNotNull{it.root}.forEach{walk(it,0)}
        File(context.filesDir,"qa-own-files-tree.json").writeText(JSONObject().put("label",label).put("nodes",items).toString(2))
        capture("qa-own-files-failure.png")
    }
    private fun selectPdfDestination() {
        click("Print destination") { it.viewIdResourceName=="com.android.printspooler:id/destination_spinner" }
        click("Save as PDF destination") { it.text?.toString()=="Save as PDF" }
    }
    private fun saveDocument(name:String) {
        val edit=node("document filename"){it.className?.toString()=="android.widget.EditText" && it.isEditable}
        assertTrue("New unique document name",edit.performAction(AccessibilityNodeInfo.ACTION_SET_TEXT,Bundle().apply{putCharSequence(AccessibilityNodeInfo.ACTION_ARGUMENT_SET_TEXT_CHARSEQUENCE,name)}))
        click("Save document") { it.isEnabled && (it.viewIdResourceName=="android:id/button1" || it.text?.toString()?.equals("Save",ignoreCase=true)==true) }
    }
    @Test fun owned_photo_upload_csv_and_print_save_pdf() {
        val values=JSONObject(File(context.filesDir,"qa-credentials.json").readText())
        val email=values.getString("email");val code=values.getString("employee_code");val run=values.getString("file_run")
        assertTrue(email.startsWith("qa-gov042-acceptance069-")&&email.endsWith("@example.invalid"))
        assertTrue(code.startsWith("QA-AT069-HIRE-"));assertTrue(Regex("[a-f0-9]{16}").matches(run))
        val imageName="HRIS-QA-$run.png";val csvName="HRIS-QA-$run.csv";val pdfName="HRIS-QA-$run.pdf"
        proof.put("csv_name",csvName).put("pdf_name",pdfName).put("image_name",imageName)
        val service=instrument.uiAutomation.serviceInfo
        service.flags=service.flags or AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS or AccessibilityServiceInfo.FLAG_RETRIEVE_INTERACTIVE_WINDOWS
        instrument.uiAutomation.serviceInfo=service
        val uri=context.contentResolver.insert(MediaStore.Images.Media.EXTERNAL_CONTENT_URI,ContentValues().apply {
            put(MediaStore.Images.Media.DISPLAY_NAME,imageName);put(MediaStore.Images.Media.MIME_TYPE,"image/png");put(MediaStore.Images.Media.RELATIVE_PATH,"Pictures/HRIS-QA");put(MediaStore.Images.Media.IS_PENDING,1)
        })?:error("Synthetic image could not be created")
        try {
            val image=Bitmap.createBitmap(128,128,Bitmap.Config.ARGB_8888)
            val canvas=Canvas(image);canvas.drawColor(Color.rgb(0,112,112));val paint=Paint(Paint.ANTI_ALIAS_FLAG).apply{color=Color.WHITE;textSize=44f}
            canvas.drawText("QA",29f,80f,paint)
            val encoded=java.io.ByteArrayOutputStream().use { buffer -> assertTrue(image.compress(Bitmap.CompressFormat.PNG,100,buffer));buffer.toByteArray() };image.recycle()
            val imageHash=java.security.MessageDigest.getInstance("SHA-256").digest(encoded).joinToString(""){"%02x".format(it)}
            context.contentResolver.openOutputStream(uri)!!.use { it.write(encoded) }
            assertEquals(1,context.contentResolver.update(uri,ContentValues().apply{put(MediaStore.Images.Media.IS_PENDING,0)},null,null))
            stage("synthetic-image-ready")
            ActivityScenario.launch(MainActivity::class.java).use { scenario ->
                await(scenario,"typeof state!=='undefined' && !state.busy && !document.querySelector('#sign-in-form button').disabled")
                evaluate(scenario,"document.getElementById('email').value="+JSONObject.quote(email)+";document.getElementById('password').value="+JSONObject.quote(values.getString("password"))+";document.getElementById('sign-in-form').requestSubmit();true")
                await(scenario,"!document.getElementById('workspace').hidden",100)
                assertEquals("true",evaluate(scenario,"state.me.employment.employee_code==="+JSONObject.quote(code)))
                evaluate(scenario,"document.querySelector('[data-page=profile]').click();true")
                await(scenario,"state.page==='profile' && !state.busy")
                assertEquals("Never replace an existing saved photo","true",evaluate(scenario,"pictureState.photo===null && pictureState.file===null && !pictureState.warning"))
                scenario.onActivity { it.browser!!.requestFocus() }
                evaluate(scenario,"document.getElementById('photo-file').focus();true")
                instrument.sendKeyDownUpSync(KeyEvent.KEYCODE_ENTER)
                node("system image picker") { it.packageName?.toString()?.let { pkg -> pkg!=context.packageName && (pkg.contains("media")||pkg.contains("documentsui")||pkg.contains("photopicker")) }==true }
                stage("system-image-picker-open")
                click("new synthetic image") {
                    val id=it.viewIdResourceName.orEmpty(); val description=it.contentDescription?.toString().orEmpty()
                    it.text?.toString()==imageName || id.endsWith("/icon_thumbnail") || id.endsWith("/thumbnail") || description.startsWith("Photo taken on")
                }
                // A single-select picker may use a confirmation button on newer system releases.
                SystemClock.sleep(500)
                find{it.text?.toString()?.let{t->t=="Add"||t=="Select"||t=="Done"}==true && it.isEnabled}?.performAction(AccessibilityNodeInfo.ACTION_CLICK)
                await(scenario,"pictureState.file!==null && !document.getElementById('photo-save').disabled")
                // Android's picker may replace the display name with a numeric media ID.
                // Verify actual selected bytes before upload, rather than trusting its label.
                evaluate(scenario,"window.qaPhotoHash=null;pictureState.file.arrayBuffer().then(b=>crypto.subtle.digest('SHA-256',b)).then(b=>window.qaPhotoHash=Array.from(new Uint8Array(b)).map(x=>x.toString(16).padStart(2,'0')).join(''));true")
                await(scenario,"window.qaPhotoHash!==null")
                assertEquals("Only the new synthetic image may be uploaded",JSONObject.quote(imageHash),evaluate(scenario,"window.qaPhotoHash"))
                proof.put("picker_returned_exact_synthetic_image",true);stage("selected-image-content-verified")
                evaluate(scenario,"document.getElementById('photo-save').click();true")
                await(scenario,"!state.busy && pictureState.photo!==null && pictureState.file===null && document.getElementById('profile-photo').naturalWidth>0",100)
                proof.put("actual_own_photo_uploaded_and_reloaded",true);stage("photo-upload-verified")
                await(scenario,"!document.getElementById('profile-photo').hidden && document.getElementById('profile-initials').hidden && document.getElementById('profile-photo').complete")
                instrument.waitForIdleSync();SystemClock.sleep(400);capture("qa-own-photo-saved.png")
                evaluate(scenario,"document.getElementById('photo-remove').click();true")
                await(scenario,"!state.busy && pictureState.photo===null",100)
                proof.put("synthetic_profile_photo_removed",true);stage("photo-cleaned")
                evaluate(scenario,"document.querySelector('[data-page=pay]').click();true")
                await(scenario,"document.querySelectorAll('#payslip-list button').length>0")
                evaluate(scenario,"document.querySelector('#payslip-list button').click();true")
                await(scenario,"!document.getElementById('payslip-detail').hidden")
                assertEquals("true",evaluate(scenario,"state.slip.employee.employee_code==="+JSONObject.quote(code)+" && state.slip.totals.net===2654245"))
                val csvExpected=evaluate(scenario,"(()=>{const s=state.slip;const rows=[['Company',s.company_name],['Pay month',s.month],['Employee code',s.employee.employee_code],['Employee',s.employee.employee_name],['Department',s.employee.department],['Rule pack',s.rule_version],...totals.map(([key,label])=>[label,(Number(s.totals[key]||0)/100).toFixed(2)]),['Payment','Approved calculation; payment recorded separately']];return String.fromCharCode(65279)+rows.map(r=>r.map(csvCell).join(',')).join(String.fromCharCode(13,10))+String.fromCharCode(13,10);})()")
                File(context.filesDir,"qa-own-csv-expected.json").writeText(csvExpected)
                evaluate(scenario,"Array.from(document.querySelectorAll('#payslip-detail button')).find(b=>b.textContent==='Download CSV').click();true")
                saveDocument(csvName)
                await(scenario,"location.origin.startsWith('http://127.0.0.1:') && !state.busy")
                node("CSV returned to app"){it.packageName?.toString()==context.packageName}
                proof.put("actual_csv_create_document_saved",true);stage("csv-saved")
                evaluate(scenario,"window.print();true")
                node("Android print preview"){it.packageName?.toString()=="com.android.printspooler"}
                selectPdfDestination()
                node("rendered Save as PDF",60){it.isEnabled && it.viewIdResourceName=="com.android.printspooler:id/print_button"}
                capture("qa-own-print-preview.png")
                click("Save as PDF"){it.isEnabled && it.viewIdResourceName=="com.android.printspooler:id/print_button"}
                saveDocument(pdfName)
                node("PDF returned to app",60){it.packageName?.toString()==context.packageName}
                proof.put("actual_android_print_pdf_saved",true);stage("pdf-saved")
                evaluate(scenario,"document.getElementById('sign-out').click();true")
                await(scenario,"!document.getElementById('sign-in-page').hidden")
                proof.put("own_session_signed_out",true);stage("complete")
            }
        } catch(e:Throwable) { if(!File(context.filesDir,"qa-own-files-tree.json").exists())dump("failed-"+proof.optString("stage"));throw e }
        finally { assertEquals("Delete only new synthetic MediaStore image",1,context.contentResolver.delete(uri,null,null)) }
    }
    @Test fun owned_print_save_pdf_only() {
        val values=JSONObject(File(context.filesDir,"qa-credentials.json").readText());val email=values.getString("email");val code=values.getString("employee_code");val run=values.getString("file_run")
        assertTrue(email.startsWith("qa-gov042-acceptance069-")&&email.endsWith("@example.invalid"));assertTrue(code.startsWith("QA-AT069-HIRE-"));assertTrue(Regex("[a-f0-9]{16}").matches(run))
        val name="HRIS-QA-$run.pdf";proof.put("pdf_name",name);stage("pdf-start")
        val service=instrument.uiAutomation.serviceInfo;service.flags=service.flags or AccessibilityServiceInfo.FLAG_REPORT_VIEW_IDS or AccessibilityServiceInfo.FLAG_RETRIEVE_INTERACTIVE_WINDOWS;instrument.uiAutomation.serviceInfo=service
        ActivityScenario.launch(MainActivity::class.java).use { scenario ->
            await(scenario,"typeof state!=='undefined' && !state.busy && !document.querySelector('#sign-in-form button').disabled")
            evaluate(scenario,"document.getElementById('email').value="+JSONObject.quote(email)+";document.getElementById('password').value="+JSONObject.quote(values.getString("password"))+";document.getElementById('sign-in-form').requestSubmit();true")
            await(scenario,"!document.getElementById('workspace').hidden",100)
            evaluate(scenario,"document.querySelector('[data-page=pay]').click();true")
            await(scenario,"document.querySelectorAll('#payslip-list button').length>0")
            evaluate(scenario,"document.querySelector('#payslip-list button').click();true")
            await(scenario,"!document.getElementById('payslip-detail').hidden")
            assertEquals("true",evaluate(scenario,"state.slip.employee.employee_code==="+JSONObject.quote(code)+" && state.slip.totals.net===2654245"))
            evaluate(scenario,"window.print();true")
            node("Android print preview"){it.packageName?.toString()=="com.android.printspooler"}
            SystemClock.sleep(1500);dump("print-preview-before-save")
            selectPdfDestination()
                node("rendered Save as PDF",60){it.isEnabled && it.viewIdResourceName=="com.android.printspooler:id/print_button"}
            capture("qa-own-print-preview.png");stage("pdf-preview-ready")
            click("Save as PDF"){it.isEnabled && it.viewIdResourceName=="com.android.printspooler:id/print_button"}
            saveDocument(name)
            node("PDF returned to app",60){it.packageName?.toString()==context.packageName}
            proof.put("actual_android_print_pdf_saved",true);stage("pdf-saved")
            evaluate(scenario,"document.getElementById('sign-out').click();true")
            await(scenario,"!document.getElementById('sign-in-page').hidden")
            proof.put("own_session_signed_out",true);stage("complete")
        }
    }

}
