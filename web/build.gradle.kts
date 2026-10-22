/*
 * Author: Adilson C. Rafael (adilson889)
 * Date: 2026-10-22
 *
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

plugins {
    kotlin("multiplatform") version "2.4.0"
}

repositories {
    mavenCentral()
}

kotlin {
    js(IR) {
        browser {
            commonWebpackConfig {
                outputFileName = "nlweb.js"
            }
        }
        binaries.executable()
    }
    sourceSets {
        val jsMain by getting {
            // the core of the Android library, unchanged
            kotlin.srcDir("../android/core/src/main/kotlin")
        }
    }
}
