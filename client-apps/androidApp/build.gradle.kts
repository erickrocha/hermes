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
val verifyReleaseSigning by tasks.registering {
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
    namespace = "com.progressrail.hermes.client.tv"
    compileSdk = 36
    buildFeatures {
        buildConfig = true
        compose = true
    }

    defaultConfig {
        applicationId = "com.progressrail.hermes.client.tv"
        minSdk = 21
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
    implementation("io.ktor:ktor-client-core:3.5.2")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.11.0")
    implementation(platform("androidx.compose:compose-bom:2025.11.01"))
    implementation("androidx.activity:activity-compose:1.11.0")
    implementation("androidx.compose.foundation:foundation")
    implementation("androidx.compose.material3:material3")
    implementation("androidx.compose.ui:ui")
    testImplementation("junit:junit:4.13.2")
    testImplementation("io.ktor:ktor-client-core:3.5.2")
    testImplementation("io.ktor:ktor-client-mock:3.5.2")
    testImplementation("io.ktor:ktor-client-content-negotiation:3.5.2")
    testImplementation("io.ktor:ktor-serialization-kotlinx-json:3.5.2")
    testImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.11.0")
}