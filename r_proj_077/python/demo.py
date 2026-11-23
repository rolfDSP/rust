"""Demonstrates and checks every feature of the `rustypy` extension module.

Build first:   maturin develop        (inside the virtualenv)
Then run:      python python/demo.py
"""

import copy
import math
import pickle
import threading
import time
from dataclasses import dataclass
from types import SimpleNamespace

import rustypy as rp


def section(title: str) -> None:
    print(f"\n=== {title} " + "=" * (60 - len(title)))


def expect_error(exc_type, func, *args, **kwargs):
    try:
        func(*args, **kwargs)
    except exc_type as e:
        print(f"  {func.__name__}{args} -> {type(e).__name__}: {e}")
        return e
    raise AssertionError(f"{func.__name__} did not raise {exc_type.__name__}")


# ---------------------------------------------------------------- module
section("Module constants")
print(f"  version={rp.__version__}  PI={rp.PI}  MAX_U32={rp.MAX_U32}  GREETING={rp.GREETING!r}")
assert rp.PI == math.pi

# --------------------------------------------------------------- scalars
section("Integers")
print("  add_i64(2, 40) =", rp.add_i64(2, 40))
print("  double_u8(200) =", rp.double_u8(200))
print("  factorial_u128(30) =", rp.factorial_u128(30))
assert rp.factorial_u128(30) == math.factorial(30)
expect_error(OverflowError, rp.double_u8, 256)          # does not fit into u8
expect_error(OverflowError, rp.add_i64, 2**63 - 1, 1)   # checked_add
expect_error(OverflowError, rp.factorial_u128, 40)
expect_error(TypeError, rp.add_i64, "1", 2)             # no implicit str->int

section("Floats / bool")
print("  hypot(3, 4) =", rp.hypot(3, 4), "(ints accepted for f64)")
print("  to_f32(0.1) =", rp.to_f32(0.1), "(precision loss)")
print("  logical_xor(True, False) =", rp.logical_xor(True, False))

section("Strings")
print("  reverse_string('Grüße 🦀') =", rp.reverse_string("Grüße 🦀"))
print("  string_stats('Grüße aus Rust 🦀') (chars, bytes, words) =", rp.string_stats("Grüße aus Rust 🦀"))
print("  next_char('a') =", rp.next_char("a"))
expect_error(ValueError, rp.next_char, "ab")             # char needs length 1

section("Bytes")
enc = rp.xor_bytes(b"secret", 0x2A)
print("  xor_bytes(b'secret', 42) =", enc, type(enc).__name__)
assert rp.xor_bytes(enc, 0x2A) == b"secret"
print("  byte_range(8) =", rp.byte_range(8))
print("  checksum(bytearray(b'abc')) =", rp.checksum(bytearray(b"abc")))

section("Complex numbers")
print("  complex_mul(1+2j, 3-1j) =", rp.complex_mul(1 + 2j, 3 - 1j))
r, phi = rp.complex_polar(1j)
print(f"  complex_polar(1j) = (r={r}, phi={phi:.4f})")
assert rp.complex_mul(1 + 2j, 3 - 1j) == (1 + 2j) * (3 - 1j)

section("Option / None")
print("  maybe_square(7) =", rp.maybe_square(7))
print("  maybe_square(None) =", rp.maybe_square(None))
print("  maybe_square() =", rp.maybe_square())
print("  do_nothing() =", rp.do_nothing())

# ----------------------------------------------------------- collections
section("Lists")
print("  stats([3, 1.5, 9, 4]) (min, max, mean) =", rp.stats([3, 1.5, 9, 4]))
print("  stats([]) =", rp.stats([]))
print("  sorted_unique((5, 3, 5, 1, 3)) =", rp.sorted_unique((5, 3, 5, 1, 3)), "(any sequence)")
print("  cross_product([1,0,0], [0,1,0]) =", rp.cross_product([1, 0, 0], [0, 1, 0]))
print("  transpose([[1,2,3],[4,5,6]]) =", rp.transpose([[1, 2, 3], [4, 5, 6]]))
expect_error(ValueError, rp.transpose, [[1, 2], [3]])
expect_error(ValueError, rp.cross_product, [1, 2], [3, 4, 5])  # wrong array length
lst = [1, 2.5, "ab", [0]]
rp.double_in_place(lst)
print("  double_in_place -> caller's list is now", lst)
assert lst == [2, 5.0, "abab", [0, 0]]

section("Tuples")
print("  describe_person(('Ada', 36, True)) =", rp.describe_person(("Ada", 36, True)))
print("  make_tuple(5) =", rp.make_tuple(5))

section("Dicts")
wc = rp.word_count("the quick brown fox jumps over The lazy dog the end")
print("  word_count(...)['the'] =", wc["the"])
print("  invert_dict({'a': 1, 'b': 2, 'c': 1}) =", rp.invert_dict({"a": 1, "b": 2, "c": 1}))
print("  system_info() =", rp.system_info())
print("  sum_numeric_values({'a': 1, 'b': 'x', 'c': 2.5, 'd': True}) =",
      rp.sum_numeric_values({"a": 1, "b": "x", "c": 2.5, "d": True}))

section("Sets")
inter, union, diff = rp.set_ops({1, 2, 3, 4}, frozenset({3, 4, 5}))
print(f"  set_ops: intersection={inter} union={union} difference={diff}")
print("  unique_chars('hello') =", sorted(rp.unique_chars("hello")))

# --------------------------------------------------------------- classes
section("Class: Point")
p = rp.Point(3, 4)
q = rp.Point(y=1.0)
print(f"  p={p}  q={q}  p.length={p.length}  abs(p)={abs(p)}")
print("  p + q =", p + q, " p - q =", p - q, " p * 2 =", p * 2, " 0.5 * p =", 0.5 * p, " -p =", -p)
print("  p / 2 =", p / 2)
expect_error(ZeroDivisionError, lambda: p / 0)
print("  p.distance_to(q) =", p.distance_to(q))
p.translate(1, 1)
p.x = 10.0                       # setter
print("  after translate + setter:", p)
print("  Point.origin() =", rp.Point.origin(), " bool(origin) =", bool(rp.Point.origin()))
print("  Point.from_polar(2, pi/2) ~", rp.Point.from_polar(2, math.pi / 2))
x, y = rp.Point(1, 2).as_tuple()
print("  unpacked as_tuple:", x, y)
assert rp.Point(1, 2) == rp.Point(1, 2) and rp.Point(1, 2) != rp.Point(2, 1)
print("  set of points:", {rp.Point(1, 2), rp.Point(1, 2), rp.Point(0, 0)})
clone = pickle.loads(pickle.dumps(rp.Point(7, 8)))
print("  pickle round trip:", clone, " copy.copy:", copy.copy(clone))
print("  centroid([...]) =", rp.centroid([rp.Point(0, 0), rp.Point(4, 0), rp.Point(2, 6)]))
expect_error(IndexError, rp.nth_point, [rp.Point()], 3)
expect_error(TypeError, rp.Point, "a", "b")

section("Class: Inventory (container protocol)")
inv = rp.Inventory({"apple": 3})
inv.add("pear")
inv.add("banana", qty=5)
inv["cherry"] = 12               # __setitem__
inv.remove("banana", 2)
print("  inv =", inv)
print("  len(inv) =", len(inv), " 'apple' in inv =", "apple" in inv, " inv['banana'] =", inv["banana"])
print("  list(inv) =", list(inv), " total =", inv.total)
del inv["pear"]                  # __delitem__
print("  after del: to_dict() =", inv.to_dict())
expect_error(KeyError, inv.__getitem__, "kiwi")

section("Class: Counter (reference semantics, iterator)")
c = rp.Counter(start=10, step=5)
alias = c
alias.increment()
print("  c.value after alias.increment() =", c.value)
print("  list(Counter(0, 3, limit=15)) =", list(rp.Counter(0, 3, limit=15)))
expect_error(AttributeError, setattr, c, "value", 1)   # read-only property

section("Inheritance")
dog = rp.Dog("Rex", ["sit"])
dog.learn("roll")
print("  dog.speak() =", dog.speak(), " tricks =", dog.tricks)
print("  isinstance(dog, Animal) =", isinstance(dog, rp.Animal))


class Cat(rp.Animal):            # Python subclass of a Rust class
    def speak(self):
        return f"{self.name} says Meow"


print("  Cat('Tom').introduce() =", Cat("Tom").introduce(), "(Rust calls the Python override)")
print("  Animal('Generic').introduce() =", rp.Animal("Generic").introduce())

# ----------------------------------------------------------------- enums
section("Enums")
print("  Color.Red =", rp.Color.Red, " int(Color.Blue) =", int(rp.Color.Blue), " hex =", rp.Color.Green.hex())
print("  Color.parse('BLUE') == Color.Blue:", rp.Color.parse("BLUE") == rp.Color.Blue)
print("  mix(Red, Blue) =", rp.mix(rp.Color.Red, rp.Color.Blue))
expect_error(ValueError, rp.Color.parse, "purple")

shapes = [
    rp.Shape.Circle(radius=1.0),
    rp.Shape.Rectangle(width=2.0, height=3.0),
    rp.Shape.Triangle(a=3.0, b=4.0, c=5.0),
    rp.Shape.Polygon([(0, 0), (4, 0), (4, 4), (0, 4)]),
]
for s in shapes:
    match s:                      # structural pattern matching works on complex enums
        case rp.Shape.Circle(radius=r):
            detail = f"r={r}"
        case rp.Shape.Rectangle(width=w, height=h):
            detail = f"{w}x{h}"
        case rp.Shape.Triangle(a=a, b=b, c=c):
            detail = f"{a}/{b}/{c}"
        case rp.Shape.Polygon():
            detail = f"{len(s[0])} vertices"
        case _:
            detail = "..."
    print(f"  {s.kind():9s} {detail:12s} area={s.area():.3f}")
print("  largest(shapes) =", rp.largest(shapes).kind())

# ---------------------------------------------------------------- errors
section("Errors / exceptions")
print("  parse_config('name = demo\\nsize = 42') =", rp.parse_config("name = demo\nsize = 42"))
expect_error(rp.ParseError, rp.parse_config, "size = 1")
expect_error(rp.ParseError, rp.parse_config, "name=a\nsize=abc")
e = expect_error(rp.LimitError, rp.parse_config, "name=a\nsize=5000")
print("  LimitError is a RustyError:", isinstance(e, rp.RustyError), " and an Exception:", isinstance(e, Exception))
expect_error(ValueError, rp.safe_sqrt, -1)
expect_error(ValueError, rp.parse_int, "12x")
print("  try_call(lambda: 1/0) =", rp.try_call(lambda: 1 / 0))
print("  try_call(lambda: 'ok') =", rp.try_call(lambda: "ok"))

# --------------------------------------------------------------- dynamic
section("Signatures: defaults, keyword-only, *args, **kwargs")
print("  ", rp.greet("Rolf"))
print("  ", rp.greet("World", "Hi", punctuation="?", times=2))
expect_error(TypeError, rp.greet, name="x")              # positional-only
expect_error(TypeError, rp.greet, "x", "Hi", "!")        # keyword-only
print(rp.summarize(1, "two", [3], three=3.0, flag=None))

section("Duck typing / union types")
for obj in [None, True, 42, 2**100, 1.5, "Grüß", [1, 2], {"a": 1}, len, (1, 2, 3), object()]:
    print(f"  type_of({obj!r:.30}) -> {rp.type_of(obj)}")
for v in [5, 2.5, " 7.25 ", (1, 4)]:
    print(f"  to_float({v!r}) = {rp.to_float(v)}")
expect_error(TypeError, rp.to_float, [1])


@dataclass
class Measurement:
    name: str
    value: float


print("  format_named(...) =", rp.format_named([Measurement("temp", 21.456), SimpleNamespace(name="hum", value=55)]))
print("  format_dicts(...) =", rp.format_dicts([{"name": "pressure", "value": 1013.25}]))
for lit in ["42", "3.14", "true", "hello", "", "[1, 2.5, x]"]:
    print(f"  parse_literal({lit!r}) = {rp.parse_literal(lit)!r}")

section("Callbacks / Python stdlib")
print("  apply(lambda x: x ** 2, [1, 2, 3]) =", rp.apply(lambda x: x**2, [1, 2, 3]))
print("  apply(str.upper, ['a', 'b']) =", rp.apply(str.upper, ["a", "b"]))
seen = []


def progress(i, total):
    seen.append(f"{i}/{total}")
    return i < 3                   # returning False stops the Rust loop


print("  run_with_progress(10, cb) =", rp.run_with_progress(10, progress), " callback saw", seen)
print("  use_stdlib(84, 36) =", rp.use_stdlib(84, 36))

section("Releasing the GIL")
N = 50_000_000
t0 = time.perf_counter()
rp.count_primes(N)
single = time.perf_counter() - t0
results = []
threads = [threading.Thread(target=lambda: results.append(rp.count_primes(N))) for _ in range(4)]
t0 = time.perf_counter()
for t in threads:
    t.start()
for t in threads:
    t.join()
parallel = time.perf_counter() - t0
print(f"  primes below {N:,}: {results[0]:,}")
print(f"  1 call: {single:.3f}s   4 calls in 4 threads: {parallel:.3f}s "
      f"(~{4 * single / parallel:.1f}x speed-up thanks to py.detach)")

print("\nAll checks passed ✔")
