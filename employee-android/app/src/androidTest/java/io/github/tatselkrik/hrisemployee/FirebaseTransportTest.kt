package io.github.tatselkrik.hrisemployee
import android.app.Notification
import androidx.test.ext.junit.runners.AndroidJUnit4
import androidx.test.platform.app.InstrumentationRegistry
import com.google.firebase.FirebaseApp
import com.google.firebase.messaging.FirebaseMessaging
import org.junit.Test
import org.junit.Assert.*
import org.junit.runner.RunWith
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import java.util.concurrent.atomic.AtomicReference
@RunWith(AndroidJUnit4::class)
class FirebaseTransportTest {
 @Test fun disabled_default_real_registration_and_private_notification_shape(){
  val context=InstrumentationRegistry.getInstrumentation().targetContext
  assertNotNull(FirebaseApp.getInstance().options.applicationId)
  val messaging=FirebaseMessaging.getInstance();assertFalse("No automatic registration before employee opt-in",messaging.isAutoInitEnabled)
  val done=CountDownLatch(1);val token=AtomicReference<String?>(null);val failure=AtomicReference<Throwable?>(null)
  messaging.token.addOnCompleteListener{task ->if(task.isSuccessful)token.set(task.result)else failure.set(task.exception);done.countDown()}
  assertTrue("Firebase registration timed out",done.await(90,TimeUnit.SECONDS));assertNull("Firebase registration must succeed",failure.get());assertTrue("Provider registration token is required",(token.get()?.length?:0)>40)
  // Do not log, export or persist this address beyond the provider-managed SDK.
  token.set(null)
  val notification=DeviceAlertNotifications.build(context)
  assertEquals("HRIS Employee",notification.extras.getString(Notification.EXTRA_TITLE))
  assertEquals("You have an HRIS update. Open the app to review your inbox.",notification.extras.getString(Notification.EXTRA_TEXT))
  assertEquals(Notification.VISIBILITY_PRIVATE,notification.visibility)
  assertNotNull(notification.contentIntent)
  assertFalse("Protected server dispatch is not enabled in this foundation",BuildConfig.PUSH_BACKEND_ENABLED)
 }
}
