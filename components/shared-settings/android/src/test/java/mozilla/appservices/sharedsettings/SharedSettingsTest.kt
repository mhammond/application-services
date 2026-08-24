/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

package mozilla.appservices.sharedsettings

import mozilla.appservices.shared_settings.SharedSettingsStore
import mozilla.appservices.syncmanager.SyncManager
import org.junit.Assert
import org.junit.Rule
import org.junit.Test
import org.junit.rules.TemporaryFolder
import org.junit.runner.RunWith
import org.robolectric.RobolectricTestRunner
import org.robolectric.annotation.Config

@RunWith(RobolectricTestRunner::class)
@Config(manifest = Config.NONE)
class SharedSettingsTest {
    @Rule
    @JvmField
    val dbFolder = TemporaryFolder()

    private fun getTestStore(): SharedSettingsStore {
        return SharedSettingsStore(dbPath = dbFolder.newFile().absolutePath)
    }

    @Test
    fun setAndGetTest() {
        val store = getTestStore()
        store.set("a-namespace", """{"a-setting": "a-value"}""")
        Assert.assertEquals("""{"a-setting":"a-value"}""", store.get("a-namespace", "null"))
    }

    @Test
    fun testRegisterWithSyncManager() {
        val syncManager = SyncManager()

        Assert.assertFalse(syncManager.getAvailableEngines().contains("shared-settings"))

        getTestStore().registerWithSyncManager()
        Assert.assertTrue(syncManager.getAvailableEngines().contains("shared-settings"))
    }
}
