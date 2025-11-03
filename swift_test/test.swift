import Foundation
import Dispatch

@main
struct TestRunner {
    static func main() {
        runTests()
    }
}

func runTests() {
    print("Testing MobiFFI Swift binding...")
    print("Using generated ergonomic API\n")

    testFreeFunctions()
    testCounter()
    testDataStore()
    testAccumulator()
    testSensorMonitorBasic()
    testCallbackTrait()
    
    print("\n=== ALL TESTS PASSED ===")
}

func testFreeFunctions() {
    print("--- Testing Free Functions ---")
    
    let greet = greeting(name: "Ali")
    print("greeting(\"Ali\") = \"\(greet)\"")
    assert(greet == "Hello, Ali!", "Expected 'Hello, Ali!'")
    
    let combined = concat(first: "Mobi", second: "FFI")
    print("concat(\"Mobi\", \"FFI\") = \"\(combined)\"")
    assert(combined == "MobiFFI", "Expected 'MobiFFI'")
    
    let reversed = reverseString(input: "Hello")
    print("reverseString(\"Hello\") = \"\(reversed)\"")
    assert(reversed == "olleH", "Expected 'olleH'")
    
    let sum = addNumbers(first: 10, second: 20)
    print("addNumbers(10, 20) = \(sum)")
    assert(sum == 30, "Expected 30")
    
    let product = multiplyFloats(first: 2.5, second: 4.0)
    print("multiplyFloats(2.5, 4.0) = \(product)")
    assert(product == 10.0, "Expected 10.0")
    
    print("SUCCESS: Free functions work!\n")
}

func testCounter() {
    print("--- Testing Counter class ---")
    
    let counter = Counter()
    counter.set(value: 10)
    print("Created counter, set to 10")
    
    var value = counter.get()
    assert(value == 10, "Initial value should be 10")
    print("Value: \(value)")
    
    counter.increment()
    counter.increment()
    counter.increment()
    value = counter.get()
    print("After 3 increments: \(value)")
    
    assert(value == 13, "Expected 13")
    print("SUCCESS: Counter works!\n")
}

func testDataStore() {
    print("--- Testing DataStore class ---")
    
    let store = DataStore()
    
    store.add(point: DataPoint(x: 1.0, y: 2.0, timestamp: 100))
    store.add(point: DataPoint(x: 3.0, y: 4.0, timestamp: 200))
    store.add(point: DataPoint(x: 5.0, y: 6.0, timestamp: 300))
    
    let count = store.len()
    print("Added 3 points, len = \(count)")
    assert(count == 3, "Expected 3 items")
    
    var collectedPoints: [DataPoint] = []
    store.foreach { point in
        collectedPoints.append(point)
    }
    print("forEach collected \(collectedPoints.count) points")
    assert(collectedPoints.count == 3, "Expected 3 points from forEach")
    assert(collectedPoints[0].x == 1.0, "First point x should be 1.0")
    assert(collectedPoints[2].x == 5.0, "Third point x should be 5.0")
    
    let sum = store.sum()
    print("sum() = \(sum)")
    let expectedSum = (1.0 + 2.0) + (3.0 + 4.0) + (5.0 + 6.0)
    assert(sum == expectedSum, "Expected sum \(expectedSum)")
    
    print("SUCCESS: DataStore works!\n")
}

func testAccumulator() {
    print("--- Testing Accumulator class ---")
    
    let acc = Accumulator()
    
    acc.add(amount: 10)
    acc.add(amount: 20)
    acc.add(amount: 5)
    
    var value = acc.get()
    print("After adding 10+20+5: \(value)")
    assert(value == 35, "Expected 35")
    
    acc.reset()
    value = acc.get()
    print("After reset: \(value)")
    assert(value == 0, "Expected 0 after reset")
    
    print("SUCCESS: Accumulator works!\n")
}

func testSensorMonitorBasic() {
    print("--- Testing SensorMonitor class ---")
    
    let monitor = SensorMonitor()
    print("Created SensorMonitor")
    
    let initialCount = monitor.subscriberCount()
    print("Initial subscriber count: \(initialCount)")
    assert(initialCount == 0, "Expected 0 initial subscribers")
    
    monitor.emitReading(sensorId: 1, timestampMs: 1000, value: 25.5)
    monitor.emitReading(sensorId: 2, timestampMs: 2000, value: 30.0)
    monitor.emitReading(sensorId: 1, timestampMs: 3000, value: 28.0)
    print("Emitted 3 readings (no subscribers yet)")
    
    print("SUCCESS: SensorMonitor works!\n")
}

func testCallbackTrait() {
    print("--- Testing Callback Trait (DataProvider) ---")
    
    class SwiftDataProvider: DataProviderProtocol {
        let data: [DataPoint]
        
        init(data: [DataPoint]) {
            self.data = data
        }
        
        func getCount() -> UInt32 {
            return UInt32(data.count)
        }
        
        func getItem(index: UInt32) -> DataPoint {
            return data[Int(index)]
        }
    }
    
    let testData = [
        DataPoint(x: 10, y: 20, timestamp: 0),
        DataPoint(x: 30, y: 40, timestamp: 0),
        DataPoint(x: 50, y: 60, timestamp: 0)
    ]
    
    let provider = SwiftDataProvider(data: testData)
    print("Created SwiftDataProvider with \(testData.count) points")
    
    let consumer = DataConsumer()
    consumer.setProvider(provider: provider)
    print("Set provider on DataConsumer")
    
    let sum = consumer.computeSum()
    print("computeSum() = \(sum)")
    
    let expectedSum: UInt64 = UInt64((10 + 20) + (30 + 40) + (50 + 60))
    assert(sum == expectedSum, "Expected \(expectedSum)")
    
    print("SUCCESS: Callback trait works! Swift -> Rust -> Swift -> Rust\n")
}
