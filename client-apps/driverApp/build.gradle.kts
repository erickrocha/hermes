plugins {
    id("com.android.application")
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
val verifyReleaseSigning by tasks.registering {
    doLast {
        val missing = releaseSigningValues.filterValues { it.isNullOrBlank() }.keys
        check(missing.isEmpty()) {
            "Release signing requires these environment variables: ${missing.joinToString()}"
        }
    }
}

val verifyReleaseApiUrl by tasks.registering {
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
    namespace = "com.progressrail.hermes.client.driver"
    compileSdk = 36
    buildFeatures {
        buildConfig = true
    }

    defaultConfig {
        applicationId = "com.progressrail.hermes.client.driver"
        minSdk = 21
        targetSdk = 36
        versionCode = 1
        versionName = "0.1.0"
        buildConfigField("String", "API_BASE_URL", "\"$apiBaseUrl\"")
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
    implementation("io.ktor:ktor-client-core:3.5.2")
    implementation("org.jetbrains.kotlinx:kotlinx-coroutines-core:1.11.0")
    implementation("androidx.activity:activity:1.11.0")
    testImplementation("junit:junit:4.13.2")
    testImplementation("io.ktor:ktor-client-core:3.5.2")
    testImplementation("io.ktor:ktor-client-mock:3.5.2")
    testImplementation("io.ktor:ktor-client-content-negotiation:3.5.2")
    testImplementation("io.ktor:ktor-serialization-kotlinx-json:3.5.2")
    testImplementation("org.jetbrains.kotlinx:kotlinx-coroutines-test:1.11.0")
}
