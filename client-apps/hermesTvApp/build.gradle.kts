plugins {
    id("com.android.application")
    id("org.jetbrains.kotlin.plugin.compose")
}

val apiBaseUrl = providers.gradleProperty("hermesApiBaseUrl")
    .orElse(providers.environmentVariable("HERMES_API_BASE_URL"))
    .getOrElse("http://10.0.2.2:8080")
val releaseSigningVariables = listOf(
    "HERMES_RELEASE_KEYSTORE",
    "HERMES_RELEASE_STORE_PASSWORD",
    "HERMES_RELEASE_KEY_ALIAS",
    "HERMES_RELEASE_KEY_PASSWORD",
)
val releaseSigningValues = releaseSigningVariables.associateWith { name ->
    providers.environmentVariable(name).orNull
}
val verifyReleaseSigning = tasks.register("verifyReleaseSigning") {
    doLast {
        val missing = releaseSigningValues.filterValues { it.isNullOrBlank() }.keys
        check(missing.isEmpty()) {
            "Release APK signing requires these environment variables: ${missing.joinToString()}"
        }
    }
}

tasks.configureEach {
    if (name == "packageRelease" || name == "bundleRelease") {
        dependsOn(verifyReleaseSigning)
    }
}

android {
    namespace = "com.erocha.hermes.client.tv"
    compileSdk = 37
    buildFeatures {
        buildConfig = true
        compose = true
    }

    defaultConfig {
        applicationId = "com.erocha.hermes.client.tv"
        minSdk = 23
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"
        buildConfigField("String", "API_BASE_URL", "\"$apiBaseUrl\"")
    }

    signingConfigs {
        create("release") {
            releaseSigningValues["HERMES_RELEASE_KEYSTORE"]?.let { storeFile = file(it) }
            storePassword = releaseSigningValues["HERMES_RELEASE_STORE_PASSWORD"]
            keyAlias = releaseSigningValues["HERMES_RELEASE_KEY_ALIAS"]
            keyPassword = releaseSigningValues["HERMES_RELEASE_KEY_PASSWORD"]
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
    implementation(platform(libs.androidx.compose.bom))
    implementation(libs.androidx.activity.compose)
    implementation(libs.androidx.compose.foundation)
    implementation(libs.androidx.compose.material3)
    implementation(libs.androidx.compose.ui)
    testImplementation(libs.junit)
    testImplementation(libs.ktor.client.core)
    testImplementation(libs.ktor.client.mock)
    testImplementation(libs.ktor.client.content.negotiation)
    testImplementation(libs.ktor.serialization.kotlinx.json)
    testImplementation(libs.kotlinx.coroutines.test)
}
