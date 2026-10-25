/*
 * Author: Adilson C. Rafael (adilson889)
 * Date: 2026-10-25
 *
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

package nativelayout.core

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertNull
import org.junit.Assert.assertTrue
import org.junit.Assert.fail
import org.junit.Test

/** `overflow-y: auto|scroll` on a box, and the <progress> element. */
class ScrollProgressTest {
    private fun build(html: String) = LayoutEngine.build(html)

    private fun errorOf(html: String): String {
        try {
            build(html)
        } catch (e: NativeLayoutError) {
            return e.message
        }
        fail("expected an error")
        return ""
    }

    // ---------- scroll ----------

    @Test
    fun overflowYAutoAndScrollMakeABoxScroll() {
        for (v in listOf("auto", "scroll")) {
            val r = build("""<div><div id="a" style="overflow-y: $v"><p>x</p><p>y</p></div></div>""")
            val a = r.byId["a"]!!
            assertTrue(a.style.scrollY)
            assertTrue(a.style.overflowHidden)
            assertTrue(r.warnings.isEmpty())
        }
        val shorthand = build("""<div><div id="a" style="overflow: auto"><p>x</p><p>y</p></div></div>""")
        assertTrue(shorthand.byId["a"]!!.style.scrollY)
    }

    @Test
    fun hiddenAndVisibleTurnItOff() {
        val hidden = build("""<div><div id="a" style="overflow-y: auto; overflow-y: hidden"><p>x</p><p>y</p></div></div>""")
        assertFalse(hidden.byId["a"]!!.style.scrollY)
        assertTrue(hidden.byId["a"]!!.style.overflowHidden)
        val visible = build("""<div><div id="a" style="overflow: auto; overflow: visible"><p>x</p><p>y</p></div></div>""")
        assertFalse(visible.byId["a"]!!.style.scrollY)
        assertFalse(visible.byId["a"]!!.style.overflowHidden)
    }

    @Test
    fun overflowXDoesNotScrollAndDoesNotUndoTheVerticalScroll() {
        val r = build("""<div><div id="a" style="overflow-y: auto; overflow-x: hidden"><p>x</p><p>y</p></div></div>""")
        assertTrue(r.byId["a"]!!.style.scrollY)
        val auto = build("""<div><div id="a" style="overflow-x: auto"><p>x</p><p>y</p></div></div>""")
        assertFalse(auto.byId["a"]!!.style.scrollY)
        assertTrue(auto.warnings.single().message.contains("overflow-x"))
    }

    @Test
    fun aFooterRegionCanScroll() {
        val f = build(
            """<div><footer id="side" style="dock: left; width: 15rem; overflow-y: auto"><p>a</p><p>b</p></footer><p>Body</p></div>"""
        ).byId["side"]!!
        assertTrue(f is NativeBox && f.footer)
        assertTrue(f.style.scrollY)
    }

    @Test
    fun theRootAndTagsThatAreNotBoxesIgnoreItWithAWarning() {
        val root = build("""<div id="r" style="overflow-y: auto"><p>x</p><p>y</p></div>""")
        assertFalse(root.byId["r"]!!.style.scrollY)
        assertTrue(root.warnings.single().message.contains("already scrolls"))

        val p = build("""<div><p id="p" style="overflow-y: auto">x</p></div>""")
        assertFalse(p.byId["p"]!!.style.scrollY)
        assertTrue(p.warnings.single().message.contains("<p>"))
    }

    @Test
    fun virtualAndAbsoluteChildrenIgnoreIt() {
        val virtual = build("""<div><div id="a" virtual style="overflow-y: auto"><p>x</p><p>y</p></div></div>""")
        assertFalse(virtual.byId["a"]!!.style.scrollY)
        assertEquals(1, virtual.warnings.size)

        val abs = build(
            """<div><div id="a" style="overflow-y: auto"><p>x</p><p style="position: absolute; top: 0">y</p></div></div>"""
        )
        assertFalse(abs.byId["a"]!!.style.scrollY)
        assertEquals(1, abs.warnings.size)
    }

    @Test
    fun invalidOverflowValueWarns() {
        val r = build("""<div><div id="a" style="overflow-y: sideways"><p>x</p><p>y</p></div></div>""")
        assertTrue(r.warnings.single().message.contains("sideways"))
    }

    @Test
    fun differReplacesTheBoxWhenTheScrollIsTurnedOnOrOff() {
        val off = build("""<div><div id="a" style="overflow-y: hidden"><p>x</p><p>y</p></div></div>""").root
        val on = build("""<div><div id="a" style="overflow-y: auto"><p>x</p><p>y</p></div></div>""").root
        val changes = LayoutDiffer.diff(off, on)
        assertTrue(changes.any { it is Change.Replace })
    }

    // ---------- progress ----------

    @Test
    fun withoutValueItIsIndeterminate() {
        val p = build("""<div><progress id="p"></progress></div>""").byId["p"] as NativeProgress
        assertNull(p.value)
        assertEquals(1f, p.max, 0f)
        assertFalse(p.circular)
        assertEquals(0f, p.fraction, 0f)
    }

    @Test
    fun valueAndMaxGiveTheFraction() {
        val p = build("""<div><progress id="p" value="3" max="12"></progress></div>""").byId["p"] as NativeProgress
        assertEquals(3f, p.value!!, 0f)
        assertEquals(0.25f, p.fraction, 0.0001f)
        val over = build("""<div><progress id="p" value="5"></progress></div>""").byId["p"] as NativeProgress
        assertEquals(1f, over.fraction, 0f)
    }

    @Test
    fun circularHasASquareDefaultSize() {
        val c = build("""<div><progress id="p" circular></progress></div>""").byId["p"] as NativeProgress
        assertTrue(c.circular)
        assertEquals(c.style.width, c.style.height)
    }

    @Test
    fun colorsComeFromColorAndBackgroundColor() {
        val p = build(
            """<div><progress id="p" value="1" style="color: #ff0000; background-color: #00ff00"></progress></div>"""
        ).byId["p"]!!
        assertEquals(0xFFFF0000.toInt(), p.style.textColor)
        assertEquals(0xFF00FF00.toInt(), p.style.backgroundColor)
    }

    @Test
    fun invalidAttributesAreErrors() {
        assertTrue(errorOf("""<div><progress value="abc"></progress></div>""").contains("value"))
        assertTrue(errorOf("""<div><progress max="0"></progress></div>""").contains("max"))
        assertTrue(errorOf("""<div><progress max="x"></progress></div>""").contains("max"))
    }

    @Test
    fun differUpdatesAttributesWhenTheValueChanges() {
        val a = build("""<div><progress id="p" value="1" max="4"></progress></div>""").root
        val b = build("""<div><progress id="p" value="2" max="4"></progress></div>""").root
        val changes = LayoutDiffer.diff(a, b)
        assertEquals(1, changes.size)
        assertTrue(changes[0] is Change.UpdateAttributes)
        assertTrue(LayoutDiffer.diff(a, a).isEmpty())
    }
}
