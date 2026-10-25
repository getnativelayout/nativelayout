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
import android.view.animation.PathInterpolator
import nativelayout.core.AnimKind
import nativelayout.core.Animation
import kotlin.math.abs
import kotlin.math.cos
import kotlin.math.exp
import kotlin.math.sin
import kotlin.math.sqrt

/**
 * Entrance motion of a screen or a <dialog> layer, with the springs and the curve of framer-motion.
 *
 * The numbers are the defaults of framer-motion: a transform (the rise of a screen, a slide) uses a spring
 * with stiffness 500 and damping 25; a scale (a layer that appears) uses stiffness 550 and damping 30;
 * opacity uses the curve cubic-bezier(.25, .1, .35, 1). The spring is the damped oscillator of its
 * `spring` generator and it runs until it is at rest (speed below 10 px/s and distance below 0.5 px),
 * not for a fixed time. The CSS still chooses the kind (`fade`, `slide-up`, `slide-left`, `none`); its
 * duration is the time of the fade.
 *
 * Everything about it is in this file. [AndroidLayoutHost.animateIn] only calls [enter].
 */
internal object ScreenMotion {
    private const val MOVE_STIFFNESS = 500f
    private const val MOVE_DAMPING = 25f
    private const val SCALE_STIFFNESS = 550f
    private const val SCALE_DAMPING = 30f

    private const val RISE_DP = 16f
    private const val SLIDE_UP_SCREEN_DP = 32f
    private const val SLIDE_UP_LAYER_DP = 96f
    private const val SLIDE_LEFT_DP = 32f

    /** A layer that fades in grows from this scale to 1. */
    private const val LAYER_SCALE_FROM = 0.96f

    private const val REST_SPEED = 10f
    private const val REST_DELTA = 0.5f
    private const val GRANULAR_SPEED = 0.01f
    private const val GRANULAR_DELTA = 0.005f
    private const val MAX_SECONDS = 10f
    private const val STEP = 1f / 120f

    /** A damped spring from 0 to 1 with no initial speed. It must be underdamped. */
    class Spring(stiffness: Float, damping: Float, mass: Float = 1f) {
        private val w0 = sqrt(stiffness / mass)
        private val zeta = damping / (2f * sqrt(stiffness * mass))
        private val wd: Float

        init {
            require(zeta > 0f && zeta < 1f) { "the spring must be underdamped" }
            wd = w0 * sqrt(1f - zeta * zeta)
        }

        fun position(t: Float): Float {
            if (t <= 0f) return 0f
            val decay = exp(-zeta * w0 * t)
            val k = zeta * w0 / wd
            return 1f - decay * (cos(wd * t) + k * sin(wd * t))
        }

        fun velocity(t: Float): Float {
            if (t <= 0f) return 0f
            return exp(-zeta * w0 * t) * (w0 * w0 / wd) * sin(wd * t)
        }

        /** Seconds until the spring is at rest when it travels [distance] (in px): the end of the motion. */
        fun restTime(distance: Float): Float {
            val d = abs(distance)
            val granular = d < 5f
            val speed = if (granular) GRANULAR_SPEED else REST_SPEED
            val delta = if (granular) GRANULAR_DELTA else REST_DELTA
            var t = STEP
            while (t < MAX_SECONDS) {
                if (abs(velocity(t)) * d <= speed && abs(1f - position(t)) * d <= delta) return t
                t += STEP
            }
            return MAX_SECONDS
        }

        /** The position over the whole motion: 0 at the start and 1 at the end. */
        fun interpolator(seconds: Float): TimeInterpolator = TimeInterpolator { x ->
            when {
                x <= 0f -> 0f
                x >= 1f -> 1f
                else -> position(x * seconds)
            }
        }
    }

    /** The curve of the opacity in framer-motion: cubic-bezier(.25, .1, .35, 1). */
    private val opacityCurve: TimeInterpolator = PathInterpolator(0.25f, 0.1f, 0.35f, 1f)

    /**
     * [view] is the whole screen or the whole layer. For a layer, the first child (the sheet or the box)
     * is what moves, so that the backdrop only fades.
     */
    fun enter(view: View, anim: Animation, layer: Boolean, density: Float) {
        val content: View = if (layer) (view as? ViewGroup)?.getChildAt(0) ?: view else view

        view.alpha = 0f
        view.animate().alpha(1f).setDuration(anim.durationMs.toLong()).setInterpolator(opacityCurve).start()

        val move: (Float) -> Unit
        val spring: Spring
        val distance: Float
        when {
            anim.kind == AnimKind.SLIDE_LEFT -> {
                val from = SLIDE_LEFT_DP * density
                content.translationX = from
                move = { p -> content.translationX = from * (1f - p) }
                spring = Spring(MOVE_STIFFNESS, MOVE_DAMPING)
                distance = from
            }
            anim.kind == AnimKind.SLIDE_UP -> {
                val from = (if (layer) SLIDE_UP_LAYER_DP else SLIDE_UP_SCREEN_DP) * density
                content.translationY = from
                move = { p -> content.translationY = from * (1f - p) }
                spring = Spring(MOVE_STIFFNESS, MOVE_DAMPING)
                distance = from
            }
            layer -> {
                content.scaleX = LAYER_SCALE_FROM
                content.scaleY = LAYER_SCALE_FROM
                move = { p ->
                    val s = LAYER_SCALE_FROM + (1f - LAYER_SCALE_FROM) * p
                    content.scaleX = s
                    content.scaleY = s
                }
                spring = Spring(SCALE_STIFFNESS, SCALE_DAMPING)
                distance = 1f - LAYER_SCALE_FROM
            }
            else -> {
                val from = RISE_DP * density
                content.translationY = from
                move = { p -> content.translationY = from * (1f - p) }
                spring = Spring(MOVE_STIFFNESS, MOVE_DAMPING)
                distance = from
            }
        }

        val seconds = spring.restTime(distance)
        ValueAnimator.ofFloat(0f, 1f).apply {
            duration = (seconds * 1000f).toLong()
            interpolator = spring.interpolator(seconds)
            addUpdateListener { move(it.animatedValue as Float) }
            start()
        }
    }
}
