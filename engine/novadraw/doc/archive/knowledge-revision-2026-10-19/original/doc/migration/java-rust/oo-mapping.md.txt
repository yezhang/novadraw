# Java 面向对象特性与 Rust 等价实现

类型：`migration-guide`

> Java 与 Rust 语法对比，一对一实现方案

---

## 1. 继承与覆盖

### Java 实现

```java
class Parent {
    void greet() { System.out.println("Hello"); }
}

class Child extends Parent {
    @Override
    void greet() { System.out.println("Hi"); }
}

void sayHello(Parent p) {
    p.greet();  // 多态调用
}
```

### Rust 等价

```rust
trait Greeter {
    fn greet(&self);
}

struct Parent;
struct Child;

impl Greeter for Parent {
    fn greet(&self) { println!("Hello"); }
}

impl Greeter for Child {
    fn greet(&self) { println!("Hi"); }
}

fn say_hello(p: &dyn Greeter) {
    p.greet();  // 动态分派
}
```

**关键点**：

- Java 用 `extends` 实现继承
- Rust 用 `trait` + `impl` 实现
- `&dyn Trait` 等价于 Java 的父类引用

---

## 2. 抽象类 / 接口

### Java 实现

```java
abstract class Shape {
    abstract double area();
}

class Circle extends Shape {
    double area() { return Math.PI * r * r; }
}
```

### Rust 等价

```rust
trait Shape {
    fn area(&self) -> f64;
}

struct Circle { r: f64 }

impl Shape for Circle {
    fn area(&self) -> f64 { std::f64::consts::PI * self.r * self.r }
}
```

**关键点**：

- Java 抽象类用 `abstract` 关键字
- Rust 用 `trait`（没有 `abstract` 关键字，方法未实现即抽象）

---

## 3. 默认方法

### Java 实现

```java
interface Drawable {
    default void draw() { System.out.println("Drawing"); }
}

class Circle implements Drawable {
    // 继承默认方法
}
```

### Rust 等价

```rust
trait Drawable {
    fn draw(&self) {
        println!("Drawing");  // 默认实现
    }
}

struct Circle;

impl Drawable for Circle {
    // 继承默认实现
}
```

**关键点**：

- Java 用 `default` 关键字
- Rust trait 方法直接提供默认实现即可

---

## 4. 多继承

### Java 实现

```java
interface Drawable { void draw(); }
interface Movable { void move(); }

class Player implements Drawable, Movable { }
```

### Rust 等价

```rust
trait Drawable { fn draw(&self); }
trait Movable { fn r#move(&self); }

struct Player;

impl Drawable for Player { fn draw(&self) {} }
impl Movable for Player { fn r#move(&self) {} }

// 组合约束
fn process<T: Drawable + Movable>(obj: &T) {}
```

**关键点**：

- Rust 用 `+` 连接多个 trait
- 注意 `move` 是关键字，需用 `r#move`

---

## 5. 泛型约束

### Java 实现

```java
<T extends Comparable<T>> T max(T a, T b) {
    return a.compareTo(b) > 0 ? a : b;
}
```

### Rust 等价

```rust
fn max<T: PartialOrd>(a: T, b: T) -> T {
    if a > b { a } else { b }
}
```

**关键点**：

- Java 用 `<T extends ...>`
- Rust 用 `<T: ...>`

---

## 6. 向上/向下转型

### Java 实现

```java
Parent p = new Child();  // 向上转型
Child c = (Child) p;     // 向下转型
```

### Rust 等价

```rust
// Rust 不支持这种转换，用 trait 对象代替

// 静态分发
let parent: Parent = Child;  // 类型转换

// 动态分发
let parent: &dyn Trait = &child;  // 转为 trait 对象

// 尝试向下转型
if let Some(c) = parent.downcast_ref::<Child>() {
    // c 是 &Child
}
```

**关键点**：

- Rust 没有继承，用 trait 对象实现多态
- 用 `Any` + `downcast_ref` 实现类似向下转型

---

## 7. 模板方法模式

### Java 实现

```java
abstract class Game {
    void play() {
        initialize();
        startPlay();
        endPlay();
    }
    abstract void initialize();
    abstract void startPlay();
}
```

### Rust 等价

```rust
trait Game {
    fn play(&self) {
        self.initialize();
        self.start_play();
        self.end_play();
    }

    fn end_play(&self) {  // 默认实现
        println!("Game ended");
    }

    fn initialize(&self);  // 抽象方法
    fn start_play(&self);  // 抽象方法
}
```

---

## 8. 策略模式

### Java 实现

```java
interface PaymentStrategy {
    void pay(int amount);
}

class CreditCardPayment implements PaymentStrategy { }

class ShoppingCart {
    PaymentStrategy strategy;
    void checkout() { strategy.pay(100); }
}
```

### Rust 等价

```rust
trait PaymentStrategy {
    fn pay(&self, amount: i32);
}

struct CreditCardPayment;

impl PaymentStrategy for CreditCardPayment {
    fn pay(&self, amount: i32) { println!("Paid {} via Card", amount); }
}

struct ShoppingCart {
    strategy: Box<dyn PaymentStrategy>,
}

impl ShoppingCart {
    fn checkout(&self) {
        self.strategy.pay(100);
    }
}
```

---

## 9. 观察者模式

### Java 实现

```java
interface Observer { void update(String msg); }

class Subject {
    List<Observer> observers = new ArrayList<>();
    void attach(Observer o) { observers.add(o); }
    void notify() { for(o : observers) o.update("changed"); }
}
```

### Rust 等价

```rust
trait Observer {
    fn update(&self, msg: &str);
}

struct Subject {
    observers: Vec<Box<dyn Observer>>,
}

impl Subject {
    fn attach(&mut self, o: Box<dyn Observer>) {
        self.observers.push(o);
    }

    fn notify(&self) {
        for o in &self.observers {
            o.update("changed");
        }
    }
}
```

---

## 10. 工厂模式

### Java 实现

```java
interface ShapeFactory {
    Shape create();
}

class CircleFactory implements ShapeFactory {
    public Shape create() { return new Circle(); }
}
```

### Rust 等价

```rust
trait ShapeFactory {
    fn create(&self) -> Box<dyn Shape>;
}

struct CircleFactory;

impl ShapeFactory for CircleFactory {
    fn create(&self) -> Box<dyn Shape> {
        Box::new(Circle)
    }
}
```

---

## 11. 单例模式

### Java 实现

```java
class Singleton {
    private static Singleton instance;
    private Singleton() {}
    public static synchronized Singleton getInstance() {
        if (instance == null) instance = new Singleton();
        return instance;
    }
}
```

### Rust 等价

```rust
// 方式1：静态变量
use std::sync::Mutex;

struct Singleton { value: i32 }

static INSTANCE: Mutex<Singleton> = Mutex::new(Singleton { value: 42 });

fn get_instance() -> std::sync::MutexGuard<'static, Singleton> {
    INSTANCE.lock().unwrap()
}

// 方式2：懒加载
use std::sync::OnceLock;

static INSTANCE: OnceLock<Singleton> = OnceLock::new();
fn get_instance() -> &'static Singleton {
    INSTANCE.get_or_init(|| Singleton { value: 42 })
}
```

---

## 12. 装饰器模式

### Java 实现

```java
interface Coffee { int cost(); }

class SimpleCoffee implements Coffee {
    public int cost() { return 10; }
}

class MilkDecorator implements Coffee {
    Coffee coffee;
    MilkDecorator(Coffee c) { this.coffee = c; }
    public int cost() { return coffee.cost() + 5; }
}
```

### Rust 等价

```rust
trait Coffee {
    fn cost(&self) -> i32;
}

struct SimpleCoffee;

impl Coffee for SimpleCoffee {
    fn cost(&self) -> i32 { 10 }
}

// 泛型装饰器
struct MilkDecorator<T: Coffee> {
    coffee: T,
}

impl<T: Coffee> Coffee for MilkDecorator<T> {
    fn cost(&self) -> i32 { self.coffee.cost() + 5 }
}
```

---

## 13. 组合模式

### Java 实现

```java
abstract class Component {
    abstract void operation();
}

class Composite extends Component {
    List<Component> children = new ArrayList<>();
    void operation() { for(c : children) c.operation(); }
}
```

### Rust 等价

```rust
trait Component {
    fn operation(&self);
}

struct Composite {
    children: Vec<Box<dyn Component>>,
}

impl Component for Composite {
    fn operation(&self) {
        for child in &self.children {
            child.operation();
        }
    }
}
```

---

## 14. 访问者模式

### Java 实现

```java
interface Visitor {
    void visit(ElementA a);
    void visit(ElementB b);
}

interface Element {
    void accept(Visitor v);
}

class ConcreteElement implements Element {
    public void accept(Visitor v) { v.visit(this); }
}
```

### Rust 等价

```rust
trait Visitor {
    fn visit_element_a(&self, e: &ElementA);
    fn visit_element_b(&self, e: &ElementB);
}

trait Element {
    fn accept(&self, v: &dyn Visitor);
}

struct ElementA;
struct ElementB;

impl Element for ElementA {
    fn accept(&self, v: &dyn Visitor) {
        v.visit_element_a(self);
    }
}
```

---

## 15. 适配器模式

### Java 实现

```java
interface Target { void request(); }
class Adaptee { void specificRequest() { } }

class Adapter implements Target {
    private Adaptee adaptee;
    public void request() { adaptee.specificRequest(); }
}
```

### Rust 等价

```rust
trait Target {
    fn request(&self);
}

struct Adaptee;

impl Adaptee {
    fn specific_request(&self) { println!("Specific"); }
}

struct Adapter { adaptee: Adaptee }

impl Target for Adapter {
    fn request(&self) {
        self.adaptee.specific_request();
    }
}
```

---

## 16. 桥接模式

### Java 实现

```java
interface Renderer { void renderCircle(); }
class VectorRenderer implements Renderer { void renderCircle() { } }

abstract class Shape { Renderer renderer; }
class Circle extends Shape { renderer.renderCircle(); }
```

### Rust 等价

```rust
trait Renderer {
    fn render_circle(&self);
}

struct VectorRenderer;
struct RasterRenderer;

impl Renderer for VectorRenderer {
    fn render_circle(&self) { println!("Vector circle"); }
}

trait Shape {
    fn draw(&self);
}

// 桥接：泛型实现
struct Circle<R: Renderer> {
    renderer: R,
}

impl<R: Renderer> Shape for Circle<R> {
    fn draw(&self) {
        self.renderer.render_circle();
    }
}
```

---

## 17. 静态方法

### Java 实现

```java
class MathUtils {
    static int add(int a, int b) { return a + b; }
}
MathUtils.add(1, 2);
```

### Rust 等价

```rust
// Rust 没有静态方法，用关联函数代替
struct MathUtils;

impl MathUtils {
    fn add(a: i32, b: i32) -> i32 {
        a + b
    }
}

MathUtils::add(1, 2);
```

---

## 18. 私有方法

### Java 实现

```java
class Calculator {
    private int helper() { return 42; }
}
```

### Rust 等价

```rust
struct Calculator;

impl Calculator {
    fn helper(&self) -> i32 { 42 }  // 默认 private
}
```

---

## 19. 多态容器

### Java 实现

```java
List<Shape> shapes = Arrays.asList(new Circle(), new Square());
for (Shape s : shapes) s.draw();
```

### Rust 等价

```rust
let shapes: Vec<Box<dyn Shape>> = vec![
    Box::new(Circle),
    Box::new(Square),
];

for s in &shapes {
    s.draw();
}
```

---

## 20. Blanket Implement

### Java 实现

```java
// Java 没有直接等价物
interface Foo { }
interface Bar { }
class Impl implements Foo, Bar { }
```

### Rust 等价

```rust
// 为所有满足条件的类型实现 trait
trait Printable {
    fn print(&self);
}

impl<T: std::fmt::Debug> Printable for T {
    fn print(&self) {
        println!("{:?}", self);
    }
}
```

---

## 总结表

| Java 特性 | Rust 等价实现 |
|-----------|---------------|
| `extends` 继承 | `trait` + `impl` |
| `implements` | `impl Trait for Type` |
| `@Override` | `impl Trait` 自动覆盖 |
| `abstract class` | `trait` + 默认方法 |
| `interface` | `trait` |
| 多继承 | `trait + trait` |
| `super.method()` | 调用默认实现 |
| 泛型 `<T>` | 泛型 `<T>` |
| `? extends T` | `T: Trait` |
| `? super T` | `&mut T` |
| `instanceof` | `Any::downcast_ref` |
| 反射 | `Any` |
| 静态方法 | 关联函数 |
| 单例 | `static` / `OnceLock` |
| 泛型方法 | `impl Trait` |
| 默认方法 | trait 默认实现 |
| 私有字段 | struct 私有字段 |
| 私有方法 | `fn` 默认 private |

---

## 关键区别

| 维度 | Java | Rust |
|------|------|------|
| 类型系统 | 编译时确定类型 | 静态分发 (`impl Trait`) 或动态 (`dyn Trait`) |
| 继承语义 | trait 是继承 | trait 是协议，不是继承 |
| 多态成本 | 有运行成本 | 静态分发零成本 |
| 扩展方式 | 类继承 | 为类型实现 trait |
| 对象创建 | `new` | `Box::new()` / 构造器模式 |

---

## trait 对象 vs 静态分发

```rust
// 动态分发：运行时多态
fn draw_dyn(shape: &dyn Drawable) {
    shape.draw();
}

// 静态分发：编译时确定
fn draw_static<T: Drawable>(shape: &T) {
    shape.draw();
}
```

---

## 参考

- Rust 官方文档：<https://doc.rust-lang.org/book/ch10-02-traits.html>
- Rust 模式与实践：<https://rust-unofficial.github.io/patterns/>
