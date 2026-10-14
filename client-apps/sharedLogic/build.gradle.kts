plugins {
    id("org.jetbrains.kotlin.multiplatform")
    id("com.android.kotlin.multiplatform.library")
}

kotlin {
    android {
        namespace = "com.progressrail.hermes.client.shared"
        compileSdk = 36
        minSdk = 21
    }

}