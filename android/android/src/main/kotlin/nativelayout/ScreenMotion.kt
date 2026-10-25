/*
 * Author: Adilson C. Rafael (adilson889)
 * Date: 2026-10-25
 *
 * This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at https://mozilla.org/MPL/2.0/.
 */

package nativelayout

import android.animation.TimeInterpolator
import android.animation.ValueAnimator
import android.view.View
import android.view.ViewGroup
import android.view.animation.DecelerateInterpolator
import nativelayout.core.AnimKind
import nativelayout.core.Animation
import kotlin.math.cos
import kotlin.math.exp
import kotlin.math.sin
import kotlin.math.sqrt

/**
 * Entrance motion of a screen or a <dialog> layer: a damped spring instead of a plain fade.
 *
 * The view fades in while its content moves into place with the overshoot of a damped spring (it goes a
 * little past its place and settles), the way a spring does in framer-motion. The CSS still chooses the
 * kind: `fade` (the default) rises a few dp, or grows a little for a layer; `slide-up` and `slide-left`
 * travel farther; `none` does nothing (that case never gets here). The duration of the CSS is the time of
 * the fade; the spring takes [SETTLE_FACTOR] times as long to settle.
 *
 * Everything about it is in this file. [AndroidLayoutHost.animateIn] only calls [enter].
 */
internal object ScreenMotion {
    /** Below 1 it overshoots a little and settles; closer to 0 it bounces more. */
    private const val DAMPING = 0.62f

    /** e^-5.3 is about 0.5%: the spring is at rest at the end of its time. */
    private const val DECAY = 5.3f

    private const val SETTLE_FACTOR = 2f
    private const val RISE_DP = 16f
    private const val SLIDE_UP_SCREEN_DP = 32f
    private const val SLIDE_UP_LAYER_DP = 96f
    private const val SLIDE_LEFT_DP = 32f

    /** A layer that fades in grows from this scale to 1. */
    private const val LAYER_SCALE_FROM = 0.96f

    /** 0 at the start and 1 at the end, with the overshoot of a damped spring in between. */
    fun spring(damping: Float = DAMPING): TimeInterpolator {
        val w0 = DECAY / damping
        val wd = w0 * sqrt(1f - damping * damping)
        val k = DECAY / wd
        return TimeInterpolator { x ->
            when {
                x <= 0f -> 0f
                x >= 1f -> 1f
                else -> 1f - exp(-DECAY * x) * (cos(wd * x) + k * sin(wd * x))
            }
        }
    }

    /**
     * [view] is the whole screen or the whole layer. For a layer, the first child (the sheet or the box)
     * is what moves, so that the backdrop only fades.
     */
    fun enter(view: View, anim: Animation, layer: Boolean, density: Float) {
        val content: View = if (layer) (view as? ViewGroup)?.getChildAt(0) ?: view else view

        view.alpha = 0f
        view.animate().alpha(1f).setDuration(anim.durationMs.toLong())
            .setInterpolator(DecelerateInterpolator()).start()

        val move: (Float) -> Unit
        when {
            anim.kind == AnimKind.SLIDE_LEFT -> {
                val from = SLIDE_LEFT_DP * density
                content.translationX = from
                move = { p -> content.translationX = from * (1f - p) }
            }
            anim.kind == AnimKind.SLIDE_UP -> {
                val from = (if (layer) SLIDE_UP_LAYER_DP else SLIDE_UP_SCREEN_DP) * density
                content.translationY = from
                move = { p -> content.translationY = from * (1f - p) }
            }
            layer -> {
                content.scaleX = LAYER_SCALE_FROM
                content.scaleY = LAYER_SCALE_FROM
                move = { p ->
                    val s = LAYER_SCALE_FROM + (1f - LAYER_SCALE_FROM) * p
                    content.scaleX = s
                    content.scaleY = s
                }
            }
            else -> {
                val from = RISE_DP * density
                content.translationY = from
                move = { p -> content.translationY = from * (1f - p) }
            }
        }

        ValueAnimator.ofFloat(0f, 1f).apply {
            duration = (anim.durationMs * SETTLE_FACTOR).toLong()
            interpolator = spring()
            addUpdateListener { move(it.animatedValue as Float) }
            start()
        }
    }
}
