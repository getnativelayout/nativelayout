/*
 * Author: Adilson C. Rafael (adilson889)
 * Date: 2026-10-15
 *
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

package nativelayout.core

sealed class Change {
    class SetRoot(val node: Native) : Change()

    class Create(val parentKey: String, val position: Int, val node: Native) : Change()
    class Remove(val key: String) : Change()

    class Replace(val key: String, val node: Native) : Change()
    class UpdateText(val key: String, val spans: List<Span>) : Change()
    class UpdateStyle(val key: String, val style: NativeStyle) : Change()

    class UpdateAttributes(val key: String, val node: Native) : Change()

    class SetItems(val key: String, val items: List<Native>) : Change()
}

object LayoutDiffer {
    fun diff(previous: Native?, next: Native): List<Change> {
        if (previous == null || previous.key != next.key || previous::class != next::class) {
            return listOf(Change.SetRoot(next))
        }
        val removed = mutableListOf<Change>()
        val rest = mutableListOf<Change>()
        compare(previous, next, removed, rest)
        return removed + rest
    }

    private fun compare(a: Native, n: Native, removed: MutableList<Change>, rest: MutableList<Change>) {
        if (a is NativeBox && n is NativeBox) {
            if (a.virtual != n.virtual || a.overlay != n.overlay || a.header != n.header || a.footer != n.footer || a.hasAbsolute != n.hasAbsolute ||
                a.style.scrollY != n.style.scrollY) {
                rest.add(Change.Replace(n.key, n))
                return
            }
            if (n.virtual) {
                if (a.style != n.style) rest.add(Change.UpdateStyle(n.key, n.style))
                if (attributesDiffer(a, n)) rest.add(Change.UpdateAttributes(n.key, n))
                if (!sameChildren(a.children, n.children)) rest.add(Change.SetItems(n.key, n.children))
                return
            }
        }
        if (a.style != n.style) rest.add(Change.UpdateStyle(n.key, n.style))
        if (attributesDiffer(a, n)) rest.add(Change.UpdateAttributes(n.key, n))
        if (a is NativeText && n is NativeText && a.spans != n.spans) {
            rest.add(Change.UpdateText(n.key, n.spans))
        }
        val childrenA = a.nativeChildren()
        val childrenN = n.nativeChildren()
        if (childrenA.isEmpty() && childrenN.isEmpty()) return
        compareChildren(n, childrenA, childrenN, removed, rest)
    }

    private fun sameChildren(a: List<Native>, b: List<Native>): Boolean {
        if (a.size != b.size) return false
        for (i in a.indices) if (!a[i].sameAs(b[i])) return false
        return true
    }

    private fun attributesDiffer(a: Native, n: Native): Boolean {
        if (a.classes != n.classes) return true
        return when {
            a is NativeButton && n is NativeButton -> a.label != n.label || a.disabled != n.disabled
            a is NativeField && n is NativeField ->
                a.type != n.type || a.hint != n.hint || a.disabled != n.disabled ||
                    a.readOnly != n.readOnly || a.rows != n.rows
            a is NativeCheckbox && n is NativeCheckbox -> a.disabled != n.disabled
            a is NativeImage && n is NativeImage -> a.source != n.source || a.description != n.description
            a is NativeProgress && n is NativeProgress ->
                a.value != n.value || a.max != n.max || a.circular != n.circular
            a is NativeItem && n is NativeItem -> a.marker != n.marker
            else -> false
        }
    }

    private fun compareChildren(
        parent: Native, previousChildren: List<Native>, nextChildren: List<Native>,
        removed: MutableList<Change>, rest: MutableList<Change>
    ) {
        val newKeys = nextChildren.map { it.key }.toSet()
        val oldByKey = previousChildren.associateBy { it.key }

        val keptOld = previousChildren.filter { it.key in newKeys }.map { it.key }
        val keptNew = nextChildren.filter { it.key in oldByKey }.map { it.key }
        if (keptOld != keptNew) {
            rest.add(Change.Replace(parent.key, parent))
            return
        }

        for (a in previousChildren) if (a.key !in newKeys) removed.add(Change.Remove(a.key))

        for ((i, n) in nextChildren.withIndex()) {
            val a = oldByKey[n.key]
            when {
                a == null -> {
                    // an absolute child lives apart from the flow: its position counts only the absolute ones
                    val absolute = n.style.position == Position.ABSOLUTE
                    val before = nextChildren.subList(0, i).count { (it.style.position == Position.ABSOLUTE) == absolute }
                    rest.add(Change.Create(parent.key, before, n))
                }
                a::class != n::class -> rest.add(Change.Replace(n.key, n))
                else -> compare(a, n, removed, rest)
            }
        }
    }
}
