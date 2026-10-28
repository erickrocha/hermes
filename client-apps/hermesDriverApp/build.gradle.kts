plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
}

val apiBaseUrl = providers.gradleProperty("hermesApiBaseUrl")
    .orElse(providers.environmentVariable("HERMES_API_BASE_URL"))
    .getOrElse("http://10.0.2.2:8080")
// The driver app is signed with its own key, distinct from the garage TV shell's.
val releaseSigningVariables = listOf(
    "HERMES_DRIVER_RELEASE_KEYSTORE",
    "HERMES_DRIVER_RELEASE_STORE_PASSWORD",
    "HERMES_DRIVER_RELEASE_KEY_ALIAS",
    "HERMES_DRIVER_RELEASE_KEY_PASSWORD",
)
val releaseSigningValues = releaseSigningVariables.associateWith { name ->
    providers.environmentVariable(name).orNull
}
val verifyReleaseSigning = tasks.register("verifyReleaseSigning") {
    doLast {
        val missing = releaseSigningValues.filterValues { it.isNullOrBlank() }.keys
        check(missing.isEmpty()) {
            "Release signing requires these environment variables: ${missing.joinToString()}"
        }
    }
}

val verifyReleaseApiUrl = tasks.register("verifyReleaseApiUrl") {
    doLast {
        check(apiBaseUrl.startsWith("https://")) { "A release build requires an https API base URL, got: $apiBaseUrl" }
    }
}

tasks.configureEach {
    if (name == "packageRelease" || name == "bundleRelease") {
        dependsOn(verifyReleaseSigning, verifyReleaseApiUrl)
    }
}

android {
    namespace = "com.erocha.hermes.client.driver"
    compileSdk = 37
    buildFeatures {
        buildConfig = true
        compose = true
    }

    defaultConfig {
        applicationId = "com.erocha.hermes.client.driver"
        minSdk = 23
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"
        buildConfigField("String", "API_BASE_URL", "\"$apiBaseUrl\"")
        // The recognizer ships native code per ABI; 32-bit phones are out of scope (arm64 phones and the x86_64 emulator).
        ndk { abiFilters += listOf("arm64-v8a", "x86_64") }
    }

    signingConfigs {
        create("release") {
            releaseSigningValues["HERMES_DRIVER_RELEASE_KEYSTORE"]?.let { storeFile = file(it) }
            storePassword = releaseSigningValues["HERMES_DRIVER_RELEASE_STORE_PASSWORD"]
            keyAlias = releaseSigningValues["HERMES_DRIVER_RELEASE_KEY_ALIAS"]
            keyPassword = releaseSigningValues["HERMES_DRIVER_RELEASE_KEY_PASSWORD"]
        }
    }

    buildTypes {
        getByName("release") {
            signingConfig = signingConfigs.getByName("release")
        }
    }
}

dependencies {
    implementation(project(":sharedLogic"))
    implementation(libs.ktor.client.core)
    implementation(libs.kotlinx.coroutines.core)
    implementation(libs.androidx.activity)
    implementation(platform(libs.androidx.compose.bom))
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.compose.foundation)
    implementation(libs.androidx.compose.material3)
    implementation(libs.androidx.compose.ui)
    // On-device text recognition for the fuel receipt: the photo never leaves the phone.
    implementation(libs.mlkit.text.recognition)
    testImplementation(libs.junit)
    testImplementation(libs.ktor.client.core)
    testImplementation(libs.ktor.client.mock)
    testImplementation(libs.ktor.client.content.negotiation)
    testImplementation(libs.ktor.serialization.kotlinx.json)
    testImplementation(libs.kotlinx.coroutines.test)
}
