plugins { id("com.android.application"); id("org.jetbrains.kotlin.plugin.compose"); id("com.google.gms.google-services") }
android {
    namespace = "io.github.tatselkrik.hrisemployee"
    compileSdk = 37
    compileSdkMinor = 2
    buildToolsVersion = "37.0.0"
    ndkVersion = "30.0.16248370"
    defaultConfig {
        applicationId = "io.github.tatselkrik.hrisemployee"
        minSdk = 26; targetSdk = 37
        versionCode = 3; versionName = "0.1.2"
        buildConfigField("boolean", "PUSH_BACKEND_ENABLED", "true")
        testInstrumentationRunner = "androidx.test.runner.AndroidJUnitRunner"
        ndk { abiFilters += listOf("arm64-v8a", "x86_64") }
    }
    buildFeatures { compose = true; buildConfig = true }
    compileOptions { sourceCompatibility = JavaVersion.VERSION_17; targetCompatibility = JavaVersion.VERSION_17 }
    packaging { jniLibs { useLegacyPackaging = false } }
    signingConfigs {
        create("privateEmployee") {
            storeFile = file(System.getenv("HRIS_ANDROID_KEYSTORE") ?: "missing-private-signing-key.p12")
            storePassword = System.getenv("HRIS_ANDROID_SIGNING_PASSWORD")
            keyAlias = "hris-employee"
            keyPassword = System.getenv("HRIS_ANDROID_SIGNING_PASSWORD")
        }
    }
    buildTypes {
        debug { signingConfig = signingConfigs.getByName("privateEmployee") }
        release { signingConfig = signingConfigs.getByName("privateEmployee"); isMinifyEnabled = true; isShrinkResources = true; proguardFiles(getDefaultProguardFile("proguard-android-optimize.txt"), "proguard-rules.pro") }
    }
}
kotlin { compilerOptions { jvmTarget.set(org.jetbrains.kotlin.gradle.dsl.JvmTarget.JVM_17) } }
dependencies {
    implementation(platform("com.google.firebase:firebase-bom:35.1.0"))
    implementation("com.google.firebase:firebase-messaging")
    val composeBom = platform("androidx.compose:compose-bom:2026.09.00")
    implementation(composeBom)
    implementation("androidx.activity:activity-compose:1.13.0")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.ui:ui")
    implementation("androidx.webkit:webkit:1.18.0")
    implementation("androidx.lifecycle:lifecycle-runtime:2.11.0")
    implementation("androidx.fragment:fragment:1.9.1")
    testImplementation("junit:junit:4.13.2")
    androidTestImplementation("androidx.test:runner:1.7.0")
    androidTestImplementation("androidx.test.ext:junit:1.3.0")
}
dependencyLocking { lockAllConfigurations() }

val verifyCompanyPackage = tasks.register("verifyCompanyPackage") {
    doLast {
        check(file("src/main/assets/hris-connection.json").isFile) { "Prepare the company deployment using tools/build-employee-android.py" }
        listOf("arm64-v8a", "x86_64").forEach { abi ->
            val library = file("src/main/jniLibs/$abi/libhris_employee_android.so")
            check(library.isFile && library.length() > 100000) { "Build both native Rust libraries before packaging" }
        }
    }
}
tasks.named("preBuild").configure { dependsOn(verifyCompanyPackage) }
