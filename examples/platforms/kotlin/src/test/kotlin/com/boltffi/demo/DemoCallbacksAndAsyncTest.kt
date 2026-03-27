package com.boltffi.demo

import kotlinx.coroutines.runBlocking
import kotlinx.coroutines.withTimeout
import kotlin.test.Test
import kotlin.test.assertContentEquals
import kotlin.test.assertEquals
import kotlin.test.assertFailsWith
import kotlin.test.assertNull
import kotlin.test.assertTrue

class DemoCallbacksAndAsyncTest {
    @Test
    fun unaryClosureExportsInvokeKotlinClosuresCorrectly() {
        var observedValue: Int? = null

        assertEquals(10, applyClosure(ClosureI32ToI32 { it * 2 }, 5))
        applyVoidClosure(ClosureI32 { observedValue = it }, 42)
        assertEquals(42, observedValue)
        assertEquals(99, applyNullaryClosure(ClosureToI32 { 99 }))
        assertEquals("HELLO", applyStringClosure(ClosureStringToString { it.uppercase() }, "hello"))
        assertEquals(false, applyBoolClosure(ClosureBoolToBool { !it }, true))
        assertDoubleEquals(9.0, applyF64Closure(ClosureF64ToF64 { it * it }, 3.0))
        assertContentEquals(intArrayOf(2, 4, 6), mapVecWithClosure(ClosureI32ToI32 { it * 2 }, intArrayOf(1, 2, 3)))
        assertContentEquals(
            intArrayOf(2, 4),
            filterVecWithClosure(ClosureI32ToBool { it % 2 == 0 }, intArrayOf(1, 2, 3, 4))
        )
        assertEquals(3L, applyOffsetClosure(ClosureISizeUSizeToISize { value, delta -> value + delta.toLong() }, -5L, 8uL))
        assertEquals(Status.PENDING, applyStatusClosure(ClosureStatusToStatus {
            if (it == Status.ACTIVE) Status.PENDING else Status.ACTIVE
        }, Status.ACTIVE))
        assertPointEquals(
            3.0,
            5.0,
            applyOptionalPointClosure(
                ClosureOptPointToOptPoint { point -> point?.let { Point(it.x + 2.0, it.y + 3.0) } },
                Point(1.0, 2.0)
            )!!
        )
        assertEquals(null, applyOptionalPointClosure(ClosureOptPointToOptPoint { it }, null))
        assertEquals(24, applyResultClosure(ClosureI32ToResultI32ErrMathError { value ->
            if (value < 0) {
                throw MathError.NegativeInput
            }
            value * 4
        }, 6))
        assertEquals(
            MathError.NegativeInput,
            assertFailsWith<MathError> {
                applyResultClosure(ClosureI32ToResultI32ErrMathError { throw MathError.NegativeInput }, -1)
            }
        )
    }

    @Test
    fun binaryAndPointClosureExportsInvokeKotlinClosuresCorrectly() {
        assertEquals(7, applyBinaryClosure(ClosureI32I32ToI32 { left, right -> left + right }, 3, 4))
        assertPointEquals(2.0, 3.0, applyPointClosure(ClosurePointToPoint { Point(it.x + 1.0, it.y + 1.0) }, Point(1.0, 2.0)))
    }

    @Test
    fun scalarSynchronousCallbackTraitsUseTheCorrectBridgeConversions() {
        val doubler = object : ValueCallback {
            override fun onValue(value: Int): Int = value * 2
        }
        val tripler = object : ValueCallback {
            override fun onValue(value: Int): Int = value * 3
        }
        val incrementer = makeIncrementingCallback(5)
        val pointTransformer = object : PointTransformer {
            override fun transform(point: Point): Point = Point(point.x + 10.0, point.y + 20.0)
        }
        val statusMapper = object : StatusMapper {
            override fun mapStatus(status: Status): Status = if (status == Status.PENDING) Status.ACTIVE else Status.INACTIVE
        }
        val flipper = makeStatusFlipper()
        val multiMethod = object : MultiMethodCallback {
            override fun methodA(x: Int): Int = x + 1
            override fun methodB(x: Int, y: Int): Int = x * y
            override fun methodC(): Int = 5
        }
        val optionCallback = object : OptionCallback {
            override fun findValue(key: Int): Int? = key.takeIf { it > 0 }?.times(10)
        }
        val resultCallback = object : ResultCallback {
            override fun compute(value: Int): Int {
                if (value < 0) {
                    throw MathError.NegativeInput
                }
                return value * 10
            }
        }
        val falliblePointTransformer = object : FalliblePointTransformer {
            override fun transformPoint(point: Point, status: Status): Point {
                if (status == Status.INACTIVE) {
                    throw MathError.NegativeInput
                }
                return Point(point.x + 100.0, point.y + 200.0)
            }
        }
        val offsetCallback = object : OffsetCallback {
            override fun offset(value: Long, delta: ULong): Long = value + delta.toLong()
        }
        val vecProcessor = object : VecProcessor {
            override fun process(values: IntArray): IntArray = values.map { it * it }.toIntArray()
        }

        assertEquals(8, invokeValueCallback(doubler, 4))
        assertEquals(14, invokeValueCallbackTwice(doubler, 3, 4))
        assertEquals(10, invokeBoxedValueCallback(doubler, 5))
        assertEquals(9, incrementer.onValue(4))
        assertEquals(9, invokeValueCallback(incrementer, 4))
        assertEquals(8, invokeOptionalValueCallback(doubler, 4))
        assertEquals(4, invokeOptionalValueCallback(null, 4))
        assertEquals(Status.ACTIVE, mapStatus(statusMapper, Status.PENDING))
        assertEquals(Status.INACTIVE, flipper.mapStatus(Status.ACTIVE))
        assertEquals(Status.PENDING, mapStatus(flipper, Status.INACTIVE))
        assertContentEquals(intArrayOf(1, 4, 9), processVec(vecProcessor, intArrayOf(1, 2, 3)))
        assertEquals(21, invokeMultiMethod(multiMethod, 3, 4))
        assertEquals(21, invokeMultiMethodBoxed(multiMethod, 3, 4))
        assertEquals(25, invokeTwoCallbacks(doubler, tripler, 5))
        assertEquals(70, invokeOptionCallback(optionCallback, 7))
        assertNull(invokeOptionCallback(optionCallback, 0))
        assertEquals(70, invokeResultCallback(resultCallback, 7))
        assertEquals(MathError.NegativeInput, assertFailsWith<MathError> { invokeResultCallback(resultCallback, -1) })
        assertEquals(3L, invokeOffsetCallback(offsetCallback, -5L, 8uL))
        assertEquals(14L, invokeBoxedOffsetCallback(offsetCallback, 10L, 4uL))
        assertPointEquals(102.0, 203.0, invokeFalliblePointTransformer(falliblePointTransformer, Point(2.0, 3.0), Status.ACTIVE))
        assertEquals(
            MathError.NegativeInput,
            assertFailsWith<MathError> {
                invokeFalliblePointTransformer(falliblePointTransformer, Point(2.0, 3.0), Status.INACTIVE)
            }
        )
    }

    @Test
    fun pointSynchronousCallbackTraitsUseTheCorrectBridgeConversions() {
        val pointTransformer = object : PointTransformer {
            override fun transform(point: Point): Point = Point(point.x + 10.0, point.y + 20.0)
        }

        assertPointEquals(11.0, 22.0, transformPoint(pointTransformer, Point(1.0, 2.0)))
        assertPointEquals(13.0, 24.0, transformPointBoxed(pointTransformer, Point(3.0, 4.0)))
    }

    @Test
    fun topLevelAsyncFunctionsRoundTripThroughKotlin() = runBlocking {
        withTimeout(10_000) {
            assertEquals(10, asyncAdd(3, 7))
            assertEquals("Echo: hello async", asyncEcho("hello async"))
            assertContentEquals(intArrayOf(2, 4, 6), asyncDoubleAll(intArrayOf(1, 2, 3)))
            assertEquals(5, asyncFindPositive(intArrayOf(-1, 0, 5, 3)))
            assertNull(asyncFindPositive(intArrayOf(-1, -2, -3)))
            assertEquals("a, b, c", asyncConcat(listOf("a", "b", "c")))
        }
    }

    @Test
    fun asyncResultFunctionsRoundTripThroughKotlin() = runBlocking {
        withTimeout(10_000) {
            assertEquals(5, asyncSafeDivide(10, 2))
            assertTrue(assertFailsWith<MathError> { asyncSafeDivide(1, 0) } is MathError.DivisionByZero)
            assertEquals("value_7", asyncFallibleFetch(7))
            assertMessageContains(assertFailsWith<FfiException> { asyncFallibleFetch(-1) }, "invalid key")
            assertEquals(40, asyncFindValue(4))
            assertNull(asyncFindValue(0))
            assertMessageContains(assertFailsWith<FfiException> { asyncFindValue(-1) }, "invalid key")
        }
    }

    @Test
    fun asyncCallbackTraitsRoundTripThroughKotlin() = runBlocking {
        withTimeout(10_000) {
            val asyncFetcher = object : AsyncFetcher {
                override suspend fun fetchValue(key: Int): Int = key * 100
                override suspend fun fetchString(input: String): String = input.uppercase()
            }
            val asyncOptionFetcher = object : AsyncOptionFetcher {
                override suspend fun find(key: Int): Long? = key.takeIf { it > 0 }?.toLong()?.times(1000L)
            }

            assertEquals(500, fetchWithAsyncCallback(asyncFetcher, 5))
            assertEquals("BOLTFFI", fetchStringWithAsyncCallback(asyncFetcher, "boltffi"))
            assertEquals(7_000L, invokeAsyncOptionFetcher(asyncOptionFetcher, 7))
            assertNull(invokeAsyncOptionFetcher(asyncOptionFetcher, 0))
        }
    }
}
