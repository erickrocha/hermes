plugins {
    id("com.android.application")
}

android {
    namespace = "com.progressrail.hermes.client.tv"
    compileSdk = 36

    defaultConfig {
        applicationId = "com.progressrail.hermes.client.tv"
        minSdk = 21
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"
    }
}

dependencies {
    implementation(project(":sharedLogic"))
}