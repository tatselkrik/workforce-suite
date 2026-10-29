package io.github.tatselkrik.hrisemployee

import android.app.Notification
import android.app.NotificationChannel
import android.app.NotificationManager
import android.app.PendingIntent
import android.content.Context
import android.content.Intent
import com.google.firebase.messaging.FirebaseMessagingService
import com.google.firebase.messaging.RemoteMessage

/** Data-only FCM messages carry no employee, salary, leave or case content.
 * Supabase remains authoritative after the employee opens and authenticates.
 */
class HrisMessagingService : FirebaseMessagingService() {
    override fun onMessageReceived(message: RemoteMessage) {
        // Never display sender-supplied text or private identifiers.
        if (BuildConfig.PUSH_BACKEND_ENABLED) DeviceAlertNotifications.show(this)
    }
}
internal object DeviceAlertNotifications {
    private const val CHANNEL = "hris-personal-updates"
    fun build(context: Context): Notification {
        val manager = context.getSystemService(NotificationManager::class.java)
        manager.createNotificationChannel(NotificationChannel(CHANNEL, "HRIS updates", NotificationManager.IMPORTANCE_DEFAULT))
        val intent = Intent(context, MainActivity::class.java).addFlags(Intent.FLAG_ACTIVITY_CLEAR_TOP or Intent.FLAG_ACTIVITY_SINGLE_TOP).putExtra("hris_open_inbox", true)
        val pending = PendingIntent.getActivity(context, 0, intent, PendingIntent.FLAG_UPDATE_CURRENT or PendingIntent.FLAG_IMMUTABLE)
        return Notification.Builder(context, CHANNEL).setSmallIcon(R.drawable.ic_notification)
            .setContentTitle("HRIS Employee").setContentText("You have an HRIS update. Open the app to review your inbox.")
            .setContentIntent(pending).setAutoCancel(true).setVisibility(Notification.VISIBILITY_PRIVATE)
            .setCategory(Notification.CATEGORY_STATUS).build()
    }
    fun show(context: Context) {
        val manager = context.getSystemService(NotificationManager::class.java)
        if (manager.areNotificationsEnabled()) manager.notify(1042, build(context))
    }
}
