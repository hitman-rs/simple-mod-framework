var ln = Object.create;
var Ve = Object.defineProperty;
var pn = Object.getOwnPropertyDescriptor;
var dn = Object.getOwnPropertyNames;
var yn = Object.getPrototypeOf,
    fn = Object.prototype.hasOwnProperty;
var In = (c, t) => () => (t || c((t = {
    exports: {}
}).exports, t), t.exports);
var On = (c, t, s, u) => {
    if (t && typeof t == "object" || typeof t == "function")
        for (let l of dn(t)) !fn.call(c, l) && l !== s && Ve(c, l, {
            get: () => t[l],
            enumerable: !(u = pn(t, l)) || u.enumerable
        });
    return c
};
var bn = (c, t, s) => (s = c != null ? ln(yn(c)) : {}, On(t || !c || !c.__esModule ? Ve(s, "default", {
    value: c,
    enumerable: !0
}) : s, c));
var Qe = In(i => {
    "use strict";
    Object.defineProperty(i, "__esModule", {
        value: !0
    });
    i.Type = i.JsonType = i.JavaScriptTypeBuilder = i.JsonTypeBuilder = i.TypeBuilder = i.TypeBuilderError = i.TransformEncodeBuilder = i.TransformDecodeBuilder = i.TemplateLiteralDslParser = i.TemplateLiteralGenerator = i.TemplateLiteralGeneratorError = i.TemplateLiteralFinite = i.TemplateLiteralFiniteError = i.TemplateLiteralParser = i.TemplateLiteralParserError = i.TemplateLiteralResolver = i.TemplateLiteralPattern = i.TemplateLiteralPatternError = i.UnionResolver = i.KeyArrayResolver = i.KeyArrayResolverError = i.KeyResolver = i.ObjectMap = i.Intrinsic = i.IndexedAccessor = i.TypeClone = i.TypeExtends = i.TypeExtendsResult = i.TypeExtendsError = i.ExtendsUndefined = i.TypeGuard = i.TypeGuardUnknownTypeError = i.ValueGuard = i.FormatRegistry = i.TypeBoxError = i.TypeRegistry = i.PatternStringExact = i.PatternNumberExact = i.PatternBooleanExact = i.PatternString = i.PatternNumber = i.PatternBoolean = i.Kind = i.Hint = i.Optional = i.Readonly = i.Transform = void 0;
    i.Transform = Symbol.for("TypeBox.Transform");
    i.Readonly = Symbol.for("TypeBox.Readonly");
    i.Optional = Symbol.for("TypeBox.Optional");
    i.Hint = Symbol.for("TypeBox.Hint");
    i.Kind = Symbol.for("TypeBox.Kind");
    i.PatternBoolean = "(true|false)";
    i.PatternNumber = "(0|[1-9][0-9]*)";
    i.PatternString = "(.*)";
    i.PatternBooleanExact = `^${i.PatternBoolean}$`;
    i.PatternNumberExact = `^${i.PatternNumber}$`;
    i.PatternStringExact = `^${i.PatternString}$`;
    var ke;
    (function (c) {
        let t = new Map;

        function s() {
            return new Map(t)
        }
        c.Entries = s;

        function u() {
            return t.clear()
        }
        c.Clear = u;

        function l(f) {
            return t.delete(f)
        }
        c.Delete = l;

        function T(f) {
            return t.has(f)
        }
        c.Has = T;

        function a(f, U) {
            t.set(f, U)
        }
        c.Set = a;

        function y(f) {
            return t.get(f)
        }
        c.Get = y
    })(ke || (i.TypeRegistry = ke = {}));
    var D = class extends Error {
        constructor(t) {
            super(t)
        }
    };
    i.TypeBoxError = D;
    var Xe;
    (function (c) {
        let t = new Map;

        function s() {
            return new Map(t)
        }
        c.Entries = s;

        function u() {
            return t.clear()
        }
        c.Clear = u;

        function l(f) {
            return t.delete(f)
        }
        c.Delete = l;

        function T(f) {
            return t.has(f)
        }
        c.Has = T;

        function a(f, U) {
            t.set(f, U)
        }
        c.Set = a;

        function y(f) {
            return t.get(f)
        }
        c.Get = y
    })(Xe || (i.FormatRegistry = Xe = {}));
    var I;
    (function (c) {
        function t(O) {
            return Array.isArray(O)
        }
        c.IsArray = t;

        function s(O) {
            return typeof O == "bigint"
        }
        c.IsBigInt = s;

        function u(O) {
            return typeof O == "boolean"
        }
        c.IsBoolean = u;

        function l(O) {
            return O instanceof globalThis.Date
        }
        c.IsDate = l;

        function T(O) {
            return O === null
        }
        c.IsNull = T;

        function a(O) {
            return typeof O == "number"
        }
        c.IsNumber = a;

        function y(O) {
            return typeof O == "object" && O !== null
        }
        c.IsObject = y;

        function f(O) {
            return typeof O == "string"
        }
        c.IsString = f;

        function U(O) {
            return O instanceof globalThis.Uint8Array
        }
        c.IsUint8Array = U;

        function b(O) {
            return O === void 0
        }
        c.IsUndefined = b
    })(I || (i.ValueGuard = I = {}));
    var De = class extends D { };
    i.TypeGuardUnknownTypeError = De;
    var o;
    (function (c) {
        function t(r) {
            try {
                return new RegExp(r), !0
            } catch {
                return !1
            }
        }

        function s(r) {
            if (!I.IsString(r)) return !1;
            for (let j = 0; j < r.length; j++) {
                let B = r.charCodeAt(j);
                if (B >= 7 && B <= 13 || B === 27 || B === 127) return !1
            }
            return !0
        }

        function u(r) {
            return a(r) || $(r)
        }

        function l(r) {
            return I.IsUndefined(r) || I.IsBigInt(r)
        }

        function T(r) {
            return I.IsUndefined(r) || I.IsNumber(r)
        }

        function a(r) {
            return I.IsUndefined(r) || I.IsBoolean(r)
        }

        function y(r) {
            return I.IsUndefined(r) || I.IsString(r)
        }

        function f(r) {
            return I.IsUndefined(r) || I.IsString(r) && s(r) && t(r)
        }

        function U(r) {
            return I.IsUndefined(r) || I.IsString(r) && s(r)
        }

        function b(r) {
            return I.IsUndefined(r) || $(r)
        }

        function O(r) {
            return x(r, "Any") && y(r.$id)
        }
        c.TAny = O;

        function m(r) {
            return x(r, "Array") && r.type === "array" && y(r.$id) && $(r.items) && T(r.minItems) && T(r.maxItems) && a(r.uniqueItems) && b(r.contains) && T(r.minContains) && T(r.maxContains)
        }
        c.TArray = m;

        function d(r) {
            return x(r, "AsyncIterator") && r.type === "AsyncIterator" && y(r.$id) && $(r.items)
        }
        c.TAsyncIterator = d;

        function P(r) {
            return x(r, "BigInt") && r.type === "bigint" && y(r.$id) && l(r.exclusiveMaximum) && l(r.exclusiveMinimum) && l(r.maximum) && l(r.minimum) && l(r.multipleOf)
        }
        c.TBigInt = P;

        function N(r) {
            return x(r, "Boolean") && r.type === "boolean" && y(r.$id)
        }
        c.TBoolean = N;

        function S(r) {
            return x(r, "Constructor") && r.type === "Constructor" && y(r.$id) && I.IsArray(r.parameters) && r.parameters.every(j => $(j)) && $(r.returns)
        }
        c.TConstructor = S;

        function v(r) {
            return x(r, "Date") && r.type === "Date" && y(r.$id) && T(r.exclusiveMaximumTimestamp) && T(r.exclusiveMinimumTimestamp) && T(r.maximumTimestamp) && T(r.minimumTimestamp) && T(r.multipleOfTimestamp)
        }
        c.TDate = v;

        function g(r) {
            return x(r, "Function") && r.type === "Function" && y(r.$id) && I.IsArray(r.parameters) && r.parameters.every(j => $(j)) && $(r.returns)
        }
        c.TFunction = g;

        function A(r) {
            return x(r, "Integer") && r.type === "integer" && y(r.$id) && T(r.exclusiveMaximum) && T(r.exclusiveMinimum) && T(r.maximum) && T(r.minimum) && T(r.multipleOf)
        }
        c.TInteger = A;

        function F(r) {
            return x(r, "Intersect") && !(I.IsString(r.type) && r.type !== "object") && I.IsArray(r.allOf) && r.allOf.every(j => $(j) && !ie(j)) && y(r.type) && (a(r.unevaluatedProperties) || b(r.unevaluatedProperties)) && y(r.$id)
        }
        c.TIntersect = F;

        function Te(r) {
            return x(r, "Iterator") && r.type === "Iterator" && y(r.$id) && $(r.items)
        }
        c.TIterator = Te;

        function x(r, j) {
            return h(r) && r[i.Kind] === j
        }
        c.TKindOf = x;

        function h(r) {
            return I.IsObject(r) && i.Kind in r && I.IsString(r[i.Kind])
        }
        c.TKind = h;

        function ee(r) {
            return z(r) && I.IsString(r.const)
        }
        c.TLiteralString = ee;

        function ce(r) {
            return z(r) && I.IsNumber(r.const)
        }
        c.TLiteralNumber = ce;

        function Ce(r) {
            return z(r) && I.IsBoolean(r.const)
        }
        c.TLiteralBoolean = Ce;

        function z(r) {
            return x(r, "Literal") && y(r.$id) && (I.IsBoolean(r.const) || I.IsNumber(r.const) || I.IsString(r.const))
        }
        c.TLiteral = z;

        function le(r) {
            return x(r, "Never") && I.IsObject(r.not) && Object.getOwnPropertyNames(r.not).length === 0
        }
        c.TNever = le;

        function K(r) {
            return x(r, "Not") && $(r.not)
        }
        c.TNot = K;

        function ne(r) {
            return x(r, "Null") && r.type === "null" && y(r.$id)
        }
        c.TNull = ne;

        function te(r) {
            return x(r, "Number") && r.type === "number" && y(r.$id) && T(r.exclusiveMaximum) && T(r.exclusiveMinimum) && T(r.maximum) && T(r.minimum) && T(r.multipleOf)
        }
        c.TNumber = te;

        function J(r) {
            return x(r, "Object") && r.type === "object" && y(r.$id) && I.IsObject(r.properties) && u(r.additionalProperties) && T(r.minProperties) && T(r.maxProperties) && Object.entries(r.properties).every(([j, B]) => s(j) && $(B))
        }
        c.TObject = J;

        function re(r) {
            return x(r, "Promise") && r.type === "Promise" && y(r.$id) && $(r.item)
        }
        c.TPromise = re;

        function pe(r) {
            return x(r, "Record") && r.type === "object" && y(r.$id) && u(r.additionalProperties) && I.IsObject(r.patternProperties) && (j => {
                let B = Object.getOwnPropertyNames(j.patternProperties);
                return B.length === 1 && t(B[0]) && I.IsObject(j.patternProperties) && $(j.patternProperties[B[0]])
            })(r)
        }
        c.TRecord = pe;

        function $e(r) {
            return I.IsObject(r) && i.Hint in r && r[i.Hint] === "Recursive"
        }
        c.TRecursive = $e;

        function de(r) {
            return x(r, "Ref") && y(r.$id) && I.IsString(r.$ref)
        }
        c.TRef = de;

        function ye(r) {
            return x(r, "String") && r.type === "string" && y(r.$id) && T(r.minLength) && T(r.maxLength) && f(r.pattern) && U(r.format)
        }
        c.TString = ye;

        function fe(r) {
            return x(r, "Symbol") && r.type === "symbol" && y(r.$id)
        }
        c.TSymbol = fe;

        function H(r) {
            return x(r, "TemplateLiteral") && r.type === "string" && I.IsString(r.pattern) && r.pattern[0] === "^" && r.pattern[r.pattern.length - 1] === "$"
        }
        c.TTemplateLiteral = H;

        function Ie(r) {
            return x(r, "This") && y(r.$id) && I.IsString(r.$ref)
        }
        c.TThis = Ie;

        function ie(r) {
            return I.IsObject(r) && i.Transform in r
        }
        c.TTransform = ie;

        function C(r) {
            return x(r, "Tuple") && r.type === "array" && y(r.$id) && I.IsNumber(r.minItems) && I.IsNumber(r.maxItems) && r.minItems === r.maxItems && (I.IsUndefined(r.items) && I.IsUndefined(r.additionalItems) && r.minItems === 0 || I.IsArray(r.items) && r.items.every(j => $(j)))
        }
        c.TTuple = C;

        function Oe(r) {
            return x(r, "Undefined") && r.type === "undefined" && y(r.$id)
        }
        c.TUndefined = Oe;

        function Ke(r) {
            return q(r) && r.anyOf.every(j => ee(j) || ce(j))
        }
        c.TUnionLiteral = Ke;

        function q(r) {
            return x(r, "Union") && y(r.$id) && I.IsObject(r) && I.IsArray(r.anyOf) && r.anyOf.every(j => $(j))
        }
        c.TUnion = q;

        function _(r) {
            return x(r, "Uint8Array") && r.type === "Uint8Array" && y(r.$id) && T(r.minByteLength) && T(r.maxByteLength)
        }
        c.TUint8Array = _;

        function E(r) {
            return x(r, "Unknown") && y(r.$id)
        }
        c.TUnknown = E;

        function be(r) {
            return x(r, "Unsafe")
        }
        c.TUnsafe = be;

        function oe(r) {
            return x(r, "Void") && r.type === "void" && y(r.$id)
        }
        c.TVoid = oe;

        function Fe(r) {
            return I.IsObject(r) && r[i.Readonly] === "Readonly"
        }
        c.TReadonly = Fe;

        function Ee(r) {
            return I.IsObject(r) && r[i.Optional] === "Optional"
        }
        c.TOptional = Ee;

        function $(r) {
            return I.IsObject(r) && (O(r) || m(r) || N(r) || P(r) || d(r) || S(r) || v(r) || g(r) || A(r) || F(r) || Te(r) || z(r) || le(r) || K(r) || ne(r) || te(r) || J(r) || re(r) || pe(r) || de(r) || ye(r) || fe(r) || H(r) || Ie(r) || C(r) || Oe(r) || q(r) || _(r) || E(r) || be(r) || oe(r) || h(r) && ke.Has(r[i.Kind]))
        }
        c.TSchema = $
    })(o || (i.TypeGuard = o = {}));
    var We;
    (function (c) {
        function t(s) {
            return s[i.Kind] === "Intersect" ? s.allOf.every(u => t(u)) : s[i.Kind] === "Union" ? s.anyOf.some(u => t(u)) : s[i.Kind] === "Undefined" ? !0 : s[i.Kind] === "Not" ? !t(s.not) : !1
        }
        c.Check = t
    })(We || (i.ExtendsUndefined = We = {}));
    var Ue = class extends D { };
    i.TypeExtendsError = Ue;
    var p;
    (function (c) {
        c[c.Union = 0] = "Union", c[c.True = 1] = "True", c[c.False = 2] = "False"
    })(p || (i.TypeExtendsResult = p = {}));
    var V;
    (function (c) {
        function t(e) {
            return e === p.False ? e : p.True
        }

        function s(e) {
            throw new Ue(e)
        }

        function u(e) {
            return o.TNever(e) || o.TIntersect(e) || o.TUnion(e) || o.TUnknown(e) || o.TAny(e)
        }

        function l(e, n) {
            return o.TNever(n) ? x(e, n) : o.TIntersect(n) ? g(e, n) : o.TUnion(n) ? Be(e, n) : o.TUnknown(n) ? qe(e, n) : o.TAny(n) ? T(e, n) : s("StructuralRight")
        }

        function T(e, n) {
            return p.True
        }

        function a(e, n) {
            return o.TIntersect(n) ? g(e, n) : o.TUnion(n) && n.anyOf.some(L => o.TAny(L) || o.TUnknown(L)) ? p.True : o.TUnion(n) ? p.Union : o.TUnknown(n) || o.TAny(n) ? p.True : p.Union
        }

        function y(e, n) {
            return o.TUnknown(e) ? p.False : o.TAny(e) ? p.Union : o.TNever(e) ? p.True : p.False
        }

        function f(e, n) {
            return o.TObject(n) && H(n) ? p.True : u(n) ? l(e, n) : o.TArray(n) ? t(w(e.items, n.items)) : p.False
        }

        function U(e, n) {
            return u(n) ? l(e, n) : o.TAsyncIterator(n) ? t(w(e.items, n.items)) : p.False
        }

        function b(e, n) {
            return u(n) ? l(e, n) : o.TObject(n) ? C(e, n) : o.TRecord(n) ? E(e, n) : o.TBigInt(n) ? p.True : p.False
        }

        function O(e, n) {
            return o.TLiteral(e) && I.IsBoolean(e.const) || o.TBoolean(e) ? p.True : p.False
        }

        function m(e, n) {
            return u(n) ? l(e, n) : o.TObject(n) ? C(e, n) : o.TRecord(n) ? E(e, n) : o.TBoolean(n) ? p.True : p.False
        }

        function d(e, n) {
            return u(n) ? l(e, n) : o.TObject(n) ? C(e, n) : o.TConstructor(n) ? e.parameters.length > n.parameters.length ? p.False : e.parameters.every((L, k) => t(w(n.parameters[k], L)) === p.True) ? t(w(e.returns, n.returns)) : p.False : p.False
        }

        function P(e, n) {
            return u(n) ? l(e, n) : o.TObject(n) ? C(e, n) : o.TRecord(n) ? E(e, n) : o.TDate(n) ? p.True : p.False
        }

        function N(e, n) {
            return u(n) ? l(e, n) : o.TObject(n) ? C(e, n) : o.TFunction(n) ? e.parameters.length > n.parameters.length ? p.False : e.parameters.every((L, k) => t(w(n.parameters[k], L)) === p.True) ? t(w(e.returns, n.returns)) : p.False : p.False
        }

        function S(e, n) {
            return o.TLiteral(e) && I.IsNumber(e.const) || o.TNumber(e) || o.TInteger(e) ? p.True : p.False
        }

        function v(e, n) {
            return o.TInteger(n) || o.TNumber(n) ? p.True : u(n) ? l(e, n) : o.TObject(n) ? C(e, n) : o.TRecord(n) ? E(e, n) : p.False
        }

        function g(e, n) {
            return n.allOf.every(L => w(e, L) === p.True) ? p.True : p.False
        }

        function A(e, n) {
            return e.allOf.some(L => w(L, n) === p.True) ? p.True : p.False
        }

        function F(e, n) {
            return u(n) ? l(e, n) : o.TIterator(n) ? t(w(e.items, n.items)) : p.False
        }

        function Te(e, n) {
            return o.TLiteral(n) && n.const === e.const ? p.True : u(n) ? l(e, n) : o.TObject(n) ? C(e, n) : o.TRecord(n) ? E(e, n) : o.TString(n) ? oe(e, n) : o.TNumber(n) ? z(e, n) : o.TInteger(n) ? S(e, n) : o.TBoolean(n) ? O(e, n) : p.False
        }

        function x(e, n) {
            return p.False
        }

        function h(e, n) {
            return p.True
        }

        function ee(e) {
            let [n, L] = [e, 0];
            for (; o.TNot(n);) n = n.not, L += 1;
            return L % 2 === 0 ? n : i.Type.Unknown()
        }

        function ce(e, n) {
            return o.TNot(e) ? w(ee(e), n) : o.TNot(n) ? w(e, ee(n)) : s("Invalid fallthrough for Not")
        }

        function Ce(e, n) {
            return u(n) ? l(e, n) : o.TObject(n) ? C(e, n) : o.TRecord(n) ? E(e, n) : o.TNull(n) ? p.True : p.False
        }

        function z(e, n) {
            return o.TLiteralNumber(e) || o.TNumber(e) || o.TInteger(e) ? p.True : p.False
        }

        function le(e, n) {
            return u(n) ? l(e, n) : o.TObject(n) ? C(e, n) : o.TRecord(n) ? E(e, n) : o.TInteger(n) || o.TNumber(n) ? p.True : p.False
        }

        function K(e, n) {
            return Object.getOwnPropertyNames(e.properties).length === n
        }

        function ne(e) {
            return H(e)
        }

        function te(e) {
            return K(e, 0) || K(e, 1) && "description" in e.properties && o.TUnion(e.properties.description) && e.properties.description.anyOf.length === 2 && (o.TString(e.properties.description.anyOf[0]) && o.TUndefined(e.properties.description.anyOf[1]) || o.TString(e.properties.description.anyOf[1]) && o.TUndefined(e.properties.description.anyOf[0]))
        }

        function J(e) {
            return K(e, 0)
        }

        function re(e) {
            return K(e, 0)
        }

        function pe(e) {
            return K(e, 0)
        }

        function $e(e) {
            return K(e, 0)
        }

        function de(e) {
            return H(e)
        }

        function ye(e) {
            let n = i.Type.Number();
            return K(e, 0) || K(e, 1) && "length" in e.properties && t(w(e.properties.length, n)) === p.True
        }

        function fe(e) {
            return K(e, 0)
        }

        function H(e) {
            let n = i.Type.Number();
            return K(e, 0) || K(e, 1) && "length" in e.properties && t(w(e.properties.length, n)) === p.True
        }

        function Ie(e) {
            let n = i.Type.Function([i.Type.Any()], i.Type.Any());
            return K(e, 0) || K(e, 1) && "then" in e.properties && t(w(e.properties.then, n)) === p.True
        }

        function ie(e, n) {
            return w(e, n) === p.False || o.TOptional(e) && !o.TOptional(n) ? p.False : p.True
        }

        function C(e, n) {
            return o.TUnknown(e) ? p.False : o.TAny(e) ? p.Union : o.TNever(e) || o.TLiteralString(e) && ne(n) || o.TLiteralNumber(e) && J(n) || o.TLiteralBoolean(e) && re(n) || o.TSymbol(e) && te(n) || o.TBigInt(e) && pe(n) || o.TString(e) && ne(n) || o.TSymbol(e) && te(n) || o.TNumber(e) && J(n) || o.TInteger(e) && J(n) || o.TBoolean(e) && re(n) || o.TUint8Array(e) && de(n) || o.TDate(e) && $e(n) || o.TConstructor(e) && fe(n) || o.TFunction(e) && ye(n) ? p.True : o.TRecord(e) && o.TString(q(e)) ? n[i.Hint] === "Record" ? p.True : p.False : o.TRecord(e) && o.TNumber(q(e)) ? K(n, 0) ? p.True : p.False : p.False
        }

        function Oe(e, n) {
            return u(n) ? l(e, n) : o.TRecord(n) ? E(e, n) : o.TObject(n) ? (() => {
                for (let L of Object.getOwnPropertyNames(n.properties)) {
                    if (!(L in e.properties) && !o.TOptional(n.properties[L])) return p.False;
                    if (o.TOptional(n.properties[L])) return p.True;
                    if (ie(e.properties[L], n.properties[L]) === p.False) return p.False
                }
                return p.True
            })() : p.False
        }

        function Ke(e, n) {
            return u(n) ? l(e, n) : o.TObject(n) && Ie(n) ? p.True : o.TPromise(n) ? t(w(e.item, n.item)) : p.False
        }

        function q(e) {
            return i.PatternNumberExact in e.patternProperties ? i.Type.Number() : i.PatternStringExact in e.patternProperties ? i.Type.String() : s("Unknown record key pattern")
        }

        function _(e) {
            return i.PatternNumberExact in e.patternProperties ? e.patternProperties[i.PatternNumberExact] : i.PatternStringExact in e.patternProperties ? e.patternProperties[i.PatternStringExact] : s("Unable to get record value schema")
        }

        function E(e, n) {
            let [L, k] = [q(n), _(n)];
            return o.TLiteralString(e) && o.TNumber(L) && t(w(e, k)) === p.True ? p.True : o.TUint8Array(e) && o.TNumber(L) || o.TString(e) && o.TNumber(L) || o.TArray(e) && o.TNumber(L) ? w(e, k) : o.TObject(e) ? (() => {
                for (let cn of Object.getOwnPropertyNames(e.properties))
                    if (ie(k, e.properties[cn]) === p.False) return p.False;
                return p.True
            })() : p.False
        }

        function be(e, n) {
            return u(n) ? l(e, n) : o.TObject(n) ? C(e, n) : o.TRecord(n) ? w(_(e), _(n)) : p.False
        }

        function oe(e, n) {
            return o.TLiteral(e) && I.IsString(e.const) || o.TString(e) ? p.True : p.False
        }

        function Fe(e, n) {
            return u(n) ? l(e, n) : o.TObject(n) ? C(e, n) : o.TRecord(n) ? E(e, n) : o.TString(n) ? p.True : p.False
        }

        function Ee(e, n) {
            return u(n) ? l(e, n) : o.TObject(n) ? C(e, n) : o.TRecord(n) ? E(e, n) : o.TSymbol(n) ? p.True : p.False
        }

        function $(e, n) {
            return o.TTemplateLiteral(e) ? w(M.Resolve(e), n) : o.TTemplateLiteral(n) ? w(e, M.Resolve(n)) : s("Invalid fallthrough for TemplateLiteral")
        }

        function r(e, n) {
            return o.TArray(n) && e.items !== void 0 && e.items.every(L => w(L, n.items) === p.True)
        }

        function j(e, n) {
            return o.TNever(e) ? p.True : o.TUnknown(e) ? p.False : o.TAny(e) ? p.Union : p.False
        }

        function B(e, n) {
            return u(n) ? l(e, n) : o.TObject(n) && H(n) || o.TArray(n) && r(e, n) ? p.True : o.TTuple(n) ? I.IsUndefined(e.items) && !I.IsUndefined(n.items) || !I.IsUndefined(e.items) && I.IsUndefined(n.items) ? p.False : I.IsUndefined(e.items) && !I.IsUndefined(n.items) || e.items.every((L, k) => w(L, n.items[k]) === p.True) ? p.True : p.False : p.False
        }

        function tn(e, n) {
            return u(n) ? l(e, n) : o.TObject(n) ? C(e, n) : o.TRecord(n) ? E(e, n) : o.TUint8Array(n) ? p.True : p.False
        }

        function rn(e, n) {
            return u(n) ? l(e, n) : o.TObject(n) ? C(e, n) : o.TRecord(n) ? E(e, n) : o.TVoid(n) ? un(e, n) : o.TUndefined(n) ? p.True : p.False
        }

        function Be(e, n) {
            return n.anyOf.some(L => w(e, L) === p.True) ? p.True : p.False
        }

        function on(e, n) {
            return e.anyOf.every(L => w(L, n) === p.True) ? p.True : p.False
        }

        function qe(e, n) {
            return p.True
        }

        function sn(e, n) {
            return o.TNever(n) ? x(e, n) : o.TIntersect(n) ? g(e, n) : o.TUnion(n) ? Be(e, n) : o.TAny(n) ? T(e, n) : o.TString(n) ? oe(e, n) : o.TNumber(n) ? z(e, n) : o.TInteger(n) ? S(e, n) : o.TBoolean(n) ? O(e, n) : o.TArray(n) ? y(e, n) : o.TTuple(n) ? j(e, n) : o.TObject(n) ? C(e, n) : o.TUnknown(n) ? p.True : p.False
        }

        function un(e, n) {
            return o.TUndefined(e) || o.TUndefined(e) ? p.True : p.False
        }

        function an(e, n) {
            return o.TIntersect(n) ? g(e, n) : o.TUnion(n) ? Be(e, n) : o.TUnknown(n) ? qe(e, n) : o.TAny(n) ? T(e, n) : o.TObject(n) ? C(e, n) : o.TVoid(n) ? p.True : p.False
        }

        function w(e, n) {
            return o.TTemplateLiteral(e) || o.TTemplateLiteral(n) ? $(e, n) : o.TNot(e) || o.TNot(n) ? ce(e, n) : o.TAny(e) ? a(e, n) : o.TArray(e) ? f(e, n) : o.TBigInt(e) ? b(e, n) : o.TBoolean(e) ? m(e, n) : o.TAsyncIterator(e) ? U(e, n) : o.TConstructor(e) ? d(e, n) : o.TDate(e) ? P(e, n) : o.TFunction(e) ? N(e, n) : o.TInteger(e) ? v(e, n) : o.TIntersect(e) ? A(e, n) : o.TIterator(e) ? F(e, n) : o.TLiteral(e) ? Te(e, n) : o.TNever(e) ? h(e, n) : o.TNull(e) ? Ce(e, n) : o.TNumber(e) ? le(e, n) : o.TObject(e) ? Oe(e, n) : o.TRecord(e) ? be(e, n) : o.TString(e) ? Fe(e, n) : o.TSymbol(e) ? Ee(e, n) : o.TTuple(e) ? B(e, n) : o.TPromise(e) ? Ke(e, n) : o.TUint8Array(e) ? tn(e, n) : o.TUndefined(e) ? rn(e, n) : o.TUnion(e) ? on(e, n) : o.TUnknown(e) ? sn(e, n) : o.TVoid(e) ? an(e, n) : s(`Unknown left type operand '${e[i.Kind]}'`)
        }

        function Tn(e, n) {
            return w(e, n)
        }
        c.Extends = Tn
    })(V || (i.TypeExtends = V = {}));
    var R;
    (function (c) {
        function t(f) {
            return f.map(U => T(U))
        }

        function s(f) {
            return new Date(f.getTime())
        }

        function u(f) {
            return new Uint8Array(f)
        }

        function l(f) {
            let U = Object.getOwnPropertyNames(f).reduce((O, m) => ({
                ...O,
                [m]: T(f[m])
            }), {}),
                b = Object.getOwnPropertySymbols(f).reduce((O, m) => ({
                    ...O,
                    [m]: T(f[m])
                }), {});
            return {
                ...U,
                ...b
            }
        }

        function T(f) {
            return I.IsArray(f) ? t(f) : I.IsDate(f) ? s(f) : I.IsUint8Array(f) ? u(f) : I.IsObject(f) ? l(f) : f
        }

        function a(f) {
            return f.map(U => y(U))
        }
        c.Rest = a;

        function y(f, U = {}) {
            return {
                ...T(f),
                ...U
            }
        }
        c.Type = y
    })(R || (i.TypeClone = R = {}));
    var Me;
    (function (c) {
        function t(d) {
            return d.map(P => {
                let {
                    [i.Optional]: N, ...S
                } = R.Type(P);
                return S
            })
        }

        function s(d) {
            return d.every(P => o.TOptional(P))
        }

        function u(d) {
            return d.some(P => o.TOptional(P))
        }

        function l(d) {
            return s(d.allOf) ? i.Type.Optional(i.Type.Intersect(t(d.allOf))) : d
        }

        function T(d) {
            return u(d.anyOf) ? i.Type.Optional(i.Type.Union(t(d.anyOf))) : d
        }

        function a(d) {
            return d[i.Kind] === "Intersect" ? l(d) : d[i.Kind] === "Union" ? T(d) : d
        }

        function y(d, P) {
            let N = d.allOf.reduce((S, v) => {
                let g = O(v, P);
                return g[i.Kind] === "Never" ? S : [...S, g]
            }, []);
            return a(i.Type.Intersect(N))
        }

        function f(d, P) {
            let N = d.anyOf.map(S => O(S, P));
            return a(i.Type.Union(N))
        }

        function U(d, P) {
            let N = d.properties[P];
            return I.IsUndefined(N) ? i.Type.Never() : i.Type.Union([N])
        }

        function b(d, P) {
            let N = d.items;
            if (I.IsUndefined(N)) return i.Type.Never();
            let S = N[P];
            return I.IsUndefined(S) ? i.Type.Never() : S
        }

        function O(d, P) {
            return d[i.Kind] === "Intersect" ? y(d, P) : d[i.Kind] === "Union" ? f(d, P) : d[i.Kind] === "Object" ? U(d, P) : d[i.Kind] === "Tuple" ? b(d, P) : i.Type.Never()
        }

        function m(d, P, N = {}) {
            let S = P.map(v => O(d, v.toString()));
            return a(i.Type.Union(S, N))
        }
        c.Resolve = m
    })(Me || (i.IndexedAccessor = Me = {}));
    var X;
    (function (c) {
        function t(b) {
            let [O, m] = [b.slice(0, 1), b.slice(1)];
            return `${O.toLowerCase()}${m}`
        }

        function s(b) {
            let [O, m] = [b.slice(0, 1), b.slice(1)];
            return `${O.toUpperCase()}${m}`
        }

        function u(b) {
            return b.toUpperCase()
        }

        function l(b) {
            return b.toLowerCase()
        }

        function T(b, O) {
            let m = Q.ParseExact(b.pattern);
            if (!Y.Check(m)) return {
                ...b,
                pattern: a(b.pattern, O)
            };
            let N = [...Z.Generate(m)].map(g => i.Type.Literal(g)),
                S = y(N, O),
                v = i.Type.Union(S);
            return i.Type.TemplateLiteral([v])
        }

        function a(b, O) {
            return typeof b == "string" ? O === "Uncapitalize" ? t(b) : O === "Capitalize" ? s(b) : O === "Uppercase" ? u(b) : O === "Lowercase" ? l(b) : b : b.toString()
        }

        function y(b, O) {
            if (b.length === 0) return [];
            let [m, ...d] = b;
            return [U(m, O), ...y(d, O)]
        }

        function f(b, O) {
            return o.TTemplateLiteral(b) ? T(b, O) : o.TUnion(b) ? i.Type.Union(y(b.anyOf, O)) : o.TLiteral(b) ? i.Type.Literal(a(b.const, O)) : b
        }

        function U(b, O) {
            return f(b, O)
        }
        c.Map = U
    })(X || (i.Intrinsic = X = {}));
    var W;
    (function (c) {
        function t(a, y) {
            return i.Type.Intersect(a.allOf.map(f => l(f, y)), {
                ...a
            })
        }

        function s(a, y) {
            return i.Type.Union(a.anyOf.map(f => l(f, y)), {
                ...a
            })
        }

        function u(a, y) {
            return y(a)
        }

        function l(a, y) {
            return a[i.Kind] === "Intersect" ? t(a, y) : a[i.Kind] === "Union" ? s(a, y) : a[i.Kind] === "Object" ? u(a, y) : a
        }

        function T(a, y, f) {
            return {
                ...l(R.Type(a), y),
                ...f
            }
        }
        c.Map = T
    })(W || (i.ObjectMap = W = {}));
    var Pe;
    (function (c) {
        function t(U) {
            return U[0] === "^" && U[U.length - 1] === "$" ? U.slice(1, U.length - 1) : U
        }

        function s(U, b) {
            return U.allOf.reduce((O, m) => [...O, ...a(m, b)], [])
        }

        function u(U, b) {
            let O = U.anyOf.map(m => a(m, b));
            return [...O.reduce((m, d) => d.map(P => O.every(N => N.includes(P)) ? m.add(P) : m)[0], new Set)]
        }

        function l(U, b) {
            return Object.getOwnPropertyNames(U.properties)
        }

        function T(U, b) {
            return b.includePatterns ? Object.getOwnPropertyNames(U.patternProperties) : []
        }

        function a(U, b) {
            return o.TIntersect(U) ? s(U, b) : o.TUnion(U) ? u(U, b) : o.TObject(U) ? l(U, b) : o.TRecord(U) ? T(U, b) : []
        }

        function y(U, b) {
            return [...new Set(a(U, b))]
        }
        c.ResolveKeys = y;

        function f(U) {
            return `^(${y(U, { includePatterns: !0 }).map(m => `($ {
                t(m)
            })`).join("|")})$`
        }
        c.ResolvePattern = f
    })(Pe || (i.KeyResolver = Pe = {}));
    var Re = class extends D { };
    i.KeyArrayResolverError = Re;
    var se;
    (function (c) {
        function t(s) {
            return Array.isArray(s) ? s : o.TUnionLiteral(s) ? s.anyOf.map(u => u.const.toString()) : o.TLiteral(s) ? [s.const] : o.TTemplateLiteral(s) ? (() => {
                let u = Q.ParseExact(s.pattern);
                if (!Y.Check(u)) throw new Re("Cannot resolve keys from infinite template expression");
                return [...Z.Generate(u)]
            })() : []
        }
        c.Resolve = t
    })(se || (i.KeyArrayResolver = se = {}));
    var ze;
    (function (c) {
        function* t(u) {
            for (let l of u.anyOf) l[i.Kind] === "Union" ? yield* t(l) : yield l
        }

        function s(u) {
            return i.Type.Union([...t(u)], {
                ...u
            })
        }
        c.Resolve = s
    })(ze || (i.UnionResolver = ze = {}));
    var Ne = class extends D { };
    i.TemplateLiteralPatternError = Ne;
    var me;
    (function (c) {
        function t(T) {
            throw new Ne(T)
        }

        function s(T) {
            return T.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")
        }

        function u(T, a) {
            return o.TTemplateLiteral(T) ? T.pattern.slice(1, T.pattern.length - 1) : o.TUnion(T) ? `(${T.anyOf.map(y => u(y, a)).join("|")})` : o.TNumber(T) ? `${a}${i.PatternNumber}` : o.TInteger(T) ? `${a}${i.PatternNumber}` : o.TBigInt(T) ? `${a}${i.PatternNumber}` : o.TString(T) ? `${a}${i.PatternString}` : o.TLiteral(T) ? `${a}${s(T.const.toString())}` : o.TBoolean(T) ? `${a}${i.PatternBoolean}` : t(`Unexpected Kind '${T[i.Kind]}'`)
        }

        function l(T) {
            return `^${T.map(a => u(a, "")).join("")}$`
        }
        c.Create = l
    })(me || (i.TemplateLiteralPattern = me = {}));
    var M;
    (function (c) {
        function t(s) {
            let u = Q.ParseExact(s.pattern);
            if (!Y.Check(u)) return i.Type.String();
            let l = [...Z.Generate(u)].map(T => i.Type.Literal(T));
            return i.Type.Union(l)
        }
        c.Resolve = t
    })(M || (i.TemplateLiteralResolver = M = {}));
    var ue = class extends D { };
    i.TemplateLiteralParserError = ue;
    var Q;
    (function (c) {
        function t(d, P, N) {
            return d[P] === N && d.charCodeAt(P - 1) !== 92
        }

        function s(d, P) {
            return t(d, P, "(")
        }

        function u(d, P) {
            return t(d, P, ")")
        }

        function l(d, P) {
            return t(d, P, "|")
        }

        function T(d) {
            if (!(s(d, 0) && u(d, d.length - 1))) return !1;
            let P = 0;
            for (let N = 0; N < d.length; N++)
                if (s(d, N) && (P += 1), u(d, N) && (P -= 1), P === 0 && N !== d.length - 1) return !1;
            return !0
        }

        function a(d) {
            return d.slice(1, d.length - 1)
        }

        function y(d) {
            let P = 0;
            for (let N = 0; N < d.length; N++)
                if (s(d, N) && (P += 1), u(d, N) && (P -= 1), l(d, N) && P === 0) return !0;
            return !1
        }

        function f(d) {
            for (let P = 0; P < d.length; P++)
                if (s(d, P)) return !0;
            return !1
        }

        function U(d) {
            let [P, N] = [0, 0], S = [];
            for (let g = 0; g < d.length; g++)
                if (s(d, g) && (P += 1), u(d, g) && (P -= 1), l(d, g) && P === 0) {
                    let A = d.slice(N, g);
                    A.length > 0 && S.push(O(A)), N = g + 1
                } let v = d.slice(N);
            return v.length > 0 && S.push(O(v)), S.length === 0 ? {
                type: "const",
                const: ""
            } : S.length === 1 ? S[0] : {
                type: "or",
                expr: S
            }
        }

        function b(d) {
            function P(v, g) {
                if (!s(v, g)) throw new ue("TemplateLiteralParser: Index must point to open parens");
                let A = 0;
                for (let F = g; F < v.length; F++)
                    if (s(v, F) && (A += 1), u(v, F) && (A -= 1), A === 0) return [g, F];
                throw new ue("TemplateLiteralParser: Unclosed group parens in expression")
            }

            function N(v, g) {
                for (let A = g; A < v.length; A++)
                    if (s(v, A)) return [g, A];
                return [g, v.length]
            }
            let S = [];
            for (let v = 0; v < d.length; v++)
                if (s(d, v)) {
                    let [g, A] = P(d, v), F = d.slice(g, A + 1);
                    S.push(O(F)), v = A
                } else {
                    let [g, A] = N(d, v), F = d.slice(g, A);
                    F.length > 0 && S.push(O(F)), v = A - 1
                } return S.length === 0 ? {
                    type: "const",
                    const: ""
                } : S.length === 1 ? S[0] : {
                    type: "and",
                    expr: S
                }
        }

        function O(d) {
            return T(d) ? O(a(d)) : y(d) ? U(d) : f(d) ? b(d) : {
                type: "const",
                const: d
            }
        }
        c.Parse = O;

        function m(d) {
            return O(d.slice(1, d.length - 1))
        }
        c.ParseExact = m
    })(Q || (i.TemplateLiteralParser = Q = {}));
    var xe = class extends D { };
    i.TemplateLiteralFiniteError = xe;
    var Y;
    (function (c) {
        function t(a) {
            throw new xe(a)
        }

        function s(a) {
            return a.type === "or" && a.expr.length === 2 && a.expr[0].type === "const" && a.expr[0].const === "0" && a.expr[1].type === "const" && a.expr[1].const === "[1-9][0-9]*"
        }

        function u(a) {
            return a.type === "or" && a.expr.length === 2 && a.expr[0].type === "const" && a.expr[0].const === "true" && a.expr[1].type === "const" && a.expr[1].const === "false"
        }

        function l(a) {
            return a.type === "const" && a.const === ".*"
        }

        function T(a) {
            return u(a) ? !0 : s(a) || l(a) ? !1 : a.type === "and" ? a.expr.every(y => T(y)) : a.type === "or" ? a.expr.every(y => T(y)) : a.type === "const" ? !0 : t("Unknown expression type")
        }
        c.Check = T
    })(Y || (i.TemplateLiteralFinite = Y = {}));
    var Se = class extends D { };
    i.TemplateLiteralGeneratorError = Se;
    var Z;
    (function (c) {
        function* t(a) {
            if (a.length === 1) return yield* a[0];
            for (let y of a[0])
                for (let f of t(a.slice(1))) yield `${y}${f}`
        }

        function* s(a) {
            return yield* t(a.expr.map(y => [...T(y)]))
        }

        function* u(a) {
            for (let y of a.expr) yield* T(y)
        }

        function* l(a) {
            return yield a.const
        }

        function* T(a) {
            return a.type === "and" ? yield* s(a) : a.type === "or" ? yield* u(a) : a.type === "const" ? yield* l(a) : (() => {
                throw new Se("Unknown expression")
            })()
        }
        c.Generate = T
    })(Z || (i.TemplateLiteralGenerator = Z = {}));
    var He;
    (function (c) {
        function* t(T) {
            let a = T.trim().replace(/"|'/g, "");
            return a === "boolean" ? yield i.Type.Boolean() : a === "number" ? yield i.Type.Number() : a === "bigint" ? yield i.Type.BigInt() : a === "string" ? yield i.Type.String() : yield (() => {
                let y = a.split("|").map(f => i.Type.Literal(f.trim()));
                return y.length === 0 ? i.Type.Never() : y.length === 1 ? y[0] : i.Type.Union(y)
            })()
        }

        function* s(T) {
            if (T[1] !== "{") {
                let a = i.Type.Literal("$"),
                    y = u(T.slice(1));
                return yield* [a, ...y]
            }
            for (let a = 2; a < T.length; a++)
                if (T[a] === "}") {
                    let y = t(T.slice(2, a)),
                        f = u(T.slice(a + 1));
                    return yield* [...y, ...f]
                } yield i.Type.Literal(T)
        }

        function* u(T) {
            for (let a = 0; a < T.length; a++)
                if (T[a] === "$") {
                    let y = i.Type.Literal(T.slice(0, a)),
                        f = s(T.slice(a));
                    return yield* [y, ...f]
                } yield i.Type.Literal(T)
        }

        function l(T) {
            return [...u(T)]
        }
        c.Parse = l
    })(He || (i.TemplateLiteralDslParser = He = {}));
    var ge = class {
        constructor(t) {
            this.schema = t
        }
        Decode(t) {
            return new Le(this.schema, t)
        }
    };
    i.TransformDecodeBuilder = ge;
    var Le = class {
        constructor(t, s) {
            this.schema = t, this.decode = s
        }
        Encode(t) {
            let s = R.Type(this.schema);
            return o.TTransform(s) ? (() => {
                let T = {
                    Encode: a => s[i.Transform].Encode(t(a)),
                    Decode: a => this.decode(s[i.Transform].Decode(a))
                };
                return {
                    ...s,
                    [i.Transform]: T
                }
            })() : (() => {
                let u = {
                    Decode: this.decode,
                    Encode: t
                };
                return {
                    ...s,
                    [i.Transform]: u
                }
            })()
        }
    };
    i.TransformEncodeBuilder = Le;
    var Rn = 0,
        ve = class extends D { };
    i.TypeBuilderError = ve;
    var we = class {
        Create(t) {
            return t
        }
        Throw(t) {
            throw new ve(t)
        }
        Discard(t, s) {
            return s.reduce((u, l) => {
                let {
                    [l]: T, ...a
                } = u;
                return a
            }, t)
        }
        Strict(t) {
            return JSON.parse(JSON.stringify(t))
        }
    };
    i.TypeBuilder = we;
    var ae = class extends we {
        ReadonlyOptional(t) {
            return this.Readonly(this.Optional(t))
        }
        Readonly(t) {
            return {
                ...R.Type(t),
                [i.Readonly]: "Readonly"
            }
        }
        Optional(t) {
            return {
                ...R.Type(t),
                [i.Optional]: "Optional"
            }
        }
        Any(t = {}) {
            return this.Create({
                ...t,
                [i.Kind]: "Any"
            })
        }
        Array(t, s = {}) {
            return this.Create({
                ...s,
                [i.Kind]: "Array",
                type: "array",
                items: R.Type(t)
            })
        }
        Boolean(t = {}) {
            return this.Create({
                ...t,
                [i.Kind]: "Boolean",
                type: "boolean"
            })
        }
        Capitalize(t, s = {}) {
            return {
                ...X.Map(R.Type(t), "Capitalize"),
                ...s
            }
        }
        Composite(t, s) {
            let u = i.Type.Intersect(t, {}),
                T = Pe.ResolveKeys(u, {
                    includePatterns: !1
                }).reduce((a, y) => ({
                    ...a,
                    [y]: i.Type.Index(u, [y])
                }), {});
            return i.Type.Object(T, s)
        }
        Enum(t, s = {}) {
            if (I.IsUndefined(t)) return this.Throw("Enum undefined or empty");
            let u = Object.getOwnPropertyNames(t).filter(a => isNaN(a)).map(a => t[a]),
                T = [...new Set(u)].map(a => i.Type.Literal(a));
            return this.Union(T, {
                ...s,
                [i.Hint]: "Enum"
            })
        }
        Extends(t, s, u, l, T = {}) {
            switch (V.Extends(t, s)) {
                case p.Union:
                    return this.Union([R.Type(u, T), R.Type(l, T)]);
                case p.True:
                    return R.Type(u, T);
                case p.False:
                    return R.Type(l, T)
            }
        }
        Exclude(t, s, u = {}) {
            return o.TTemplateLiteral(t) ? this.Exclude(M.Resolve(t), s, u) : o.TTemplateLiteral(s) ? this.Exclude(t, M.Resolve(s), u) : o.TUnion(t) ? (() => {
                let l = t.anyOf.filter(T => V.Extends(T, s) === p.False);
                return l.length === 1 ? R.Type(l[0], u) : this.Union(l, u)
            })() : V.Extends(t, s) !== p.False ? this.Never(u) : R.Type(t, u)
        }
        Extract(t, s, u = {}) {
            return o.TTemplateLiteral(t) ? this.Extract(M.Resolve(t), s, u) : o.TTemplateLiteral(s) ? this.Extract(t, M.Resolve(s), u) : o.TUnion(t) ? (() => {
                let l = t.anyOf.filter(T => V.Extends(T, s) !== p.False);
                return l.length === 1 ? R.Type(l[0], u) : this.Union(l, u)
            })() : V.Extends(t, s) !== p.False ? R.Type(t, u) : this.Never(u)
        }
        Index(t, s, u = {}) {
            return o.TArray(t) && o.TNumber(s) ? R.Type(t.items, u) : o.TTuple(t) && o.TNumber(s) ? (() => {
                let T = (I.IsUndefined(t.items) ? [] : t.items).map(a => R.Type(a));
                return this.Union(T, u)
            })() : (() => {
                let l = se.Resolve(s),
                    T = R.Type(t);
                return Me.Resolve(T, l, u)
            })()
        }
        Integer(t = {}) {
            return this.Create({
                ...t,
                [i.Kind]: "Integer",
                type: "integer"
            })
        }
        Intersect(t, s = {}) {
            if (t.length === 0) return i.Type.Never();
            if (t.length === 1) return R.Type(t[0], s);
            t.some(a => o.TTransform(a)) && this.Throw("Cannot intersect transform types");
            let u = t.every(a => o.TObject(a)),
                l = R.Rest(t),
                T = o.TSchema(s.unevaluatedProperties) ? {
                    unevaluatedProperties: R.Type(s.unevaluatedProperties)
                } : {};
            return s.unevaluatedProperties === !1 || o.TSchema(s.unevaluatedProperties) || u ? this.Create({
                ...s,
                ...T,
                [i.Kind]: "Intersect",
                type: "object",
                allOf: l
            }) : this.Create({
                ...s,
                ...T,
                [i.Kind]: "Intersect",
                allOf: l
            })
        }
        KeyOf(t, s = {}) {
            return o.TRecord(t) ? (() => {
                let u = Object.getOwnPropertyNames(t.patternProperties)[0];
                return u === i.PatternNumberExact ? this.Number(s) : u === i.PatternStringExact ? this.String(s) : this.Throw("Unable to resolve key type from Record key pattern")
            })() : o.TTuple(t) ? (() => {
                let l = (I.IsUndefined(t.items) ? [] : t.items).map((T, a) => i.Type.Literal(a.toString()));
                return this.Union(l, s)
            })() : o.TArray(t) ? this.Number(s) : (() => {
                let u = Pe.ResolveKeys(t, {
                    includePatterns: !1
                });
                if (u.length === 0) return this.Never(s);
                let l = u.map(T => this.Literal(T));
                return this.Union(l, s)
            })()
        }
        Literal(t, s = {}) {
            return this.Create({
                ...s,
                [i.Kind]: "Literal",
                const: t,
                type: typeof t
            })
        }
        Lowercase(t, s = {}) {
            return {
                ...X.Map(R.Type(t), "Lowercase"),
                ...s
            }
        }
        Never(t = {}) {
            return this.Create({
                ...t,
                [i.Kind]: "Never",
                not: {}
            })
        }
        Not(t, s) {
            return this.Create({
                ...s,
                [i.Kind]: "Not",
                not: R.Type(t)
            })
        }
        Null(t = {}) {
            return this.Create({
                ...t,
                [i.Kind]: "Null",
                type: "null"
            })
        }
        Number(t = {}) {
            return this.Create({
                ...t,
                [i.Kind]: "Number",
                type: "number"
            })
        }
        Object(t, s = {}) {
            let u = Object.getOwnPropertyNames(t),
                l = u.filter(f => o.TOptional(t[f])),
                T = u.filter(f => !l.includes(f)),
                a = o.TSchema(s.additionalProperties) ? {
                    additionalProperties: R.Type(s.additionalProperties)
                } : {},
                y = u.reduce((f, U) => ({
                    ...f,
                    [U]: R.Type(t[U])
                }), {});
            return T.length > 0 ? this.Create({
                ...s,
                ...a,
                [i.Kind]: "Object",
                type: "object",
                properties: y,
                required: T
            }) : this.Create({
                ...s,
                ...a,
                [i.Kind]: "Object",
                type: "object",
                properties: y
            })
        }
        Omit(t, s, u = {}) {
            let l = se.Resolve(s);
            return W.Map(this.Discard(R.Type(t), ["$id", i.Transform]), T => {
                I.IsArray(T.required) && (T.required = T.required.filter(a => !l.includes(a)), T.required.length === 0 && delete T.required);
                for (let a of Object.getOwnPropertyNames(T.properties)) l.includes(a) && delete T.properties[a];
                return this.Create(T)
            }, u)
        }
        Partial(t, s = {}) {
            return W.Map(this.Discard(R.Type(t), ["$id", i.Transform]), u => {
                let l = Object.getOwnPropertyNames(u.properties).reduce((T, a) => ({
                    ...T,
                    [a]: this.Optional(u.properties[a])
                }), {});
                return this.Object(l, this.Discard(u, ["required"]))
            }, s)
        }
        Pick(t, s, u = {}) {
            let l = se.Resolve(s);
            return W.Map(this.Discard(R.Type(t), ["$id", i.Transform]), T => {
                I.IsArray(T.required) && (T.required = T.required.filter(a => l.includes(a)), T.required.length === 0 && delete T.required);
                for (let a of Object.getOwnPropertyNames(T.properties)) l.includes(a) || delete T.properties[a];
                return this.Create(T)
            }, u)
        }
        Record(t, s, u = {}) {
            return o.TTemplateLiteral(t) ? (() => {
                let l = Q.ParseExact(t.pattern);
                return Y.Check(l) ? this.Object([...Z.Generate(l)].reduce((T, a) => ({
                    ...T,
                    [a]: R.Type(s)
                }), {}), u) : this.Create({
                    ...u,
                    [i.Kind]: "Record",
                    type: "object",
                    patternProperties: {
                        [t.pattern]: R.Type(s)
                    }
                })
            })() : o.TUnion(t) ? (() => {
                let l = ze.Resolve(t);
                if (o.TUnionLiteral(l)) {
                    let T = l.anyOf.reduce((a, y) => ({
                        ...a,
                        [y.const]: R.Type(s)
                    }), {});
                    return this.Object(T, {
                        ...u,
                        [i.Hint]: "Record"
                    })
                } else this.Throw("Record key of type union contains non-literal types")
            })() : o.TLiteral(t) ? I.IsString(t.const) || I.IsNumber(t.const) ? this.Object({
                [t.const]: R.Type(s)
            }, u) : this.Throw("Record key of type literal is not of type string or number") : o.TInteger(t) || o.TNumber(t) ? this.Create({
                ...u,
                [i.Kind]: "Record",
                type: "object",
                patternProperties: {
                    [i.PatternNumberExact]: R.Type(s)
                }
            }) : o.TString(t) ? (() => {
                let l = I.IsUndefined(t.pattern) ? i.PatternStringExact : t.pattern;
                return this.Create({
                    ...u,
                    [i.Kind]: "Record",
                    type: "object",
                    patternProperties: {
                        [l]: R.Type(s)
                    }
                })
            })() : this.Never()
        }
        Recursive(t, s = {}) {
            I.IsUndefined(s.$id) && (s.$id = `T${Rn++}`);
            let u = t({
                [i.Kind]: "This",
                $ref: `${s.$id}`
            });
            return u.$id = s.$id, this.Create({
                ...s,
                [i.Hint]: "Recursive",
                ...u
            })
        }
        Ref(t, s = {}) {
            return I.IsString(t) ? this.Create({
                ...s,
                [i.Kind]: "Ref",
                $ref: t
            }) : (I.IsUndefined(t.$id) && this.Throw("Reference target type must specify an $id"), this.Create({
                ...s,
                [i.Kind]: "Ref",
                $ref: t.$id
            }))
        }
        Required(t, s = {}) {
            return W.Map(this.Discard(R.Type(t), ["$id", i.Transform]), u => {
                let l = Object.getOwnPropertyNames(u.properties).reduce((T, a) => ({
                    ...T,
                    [a]: this.Discard(u.properties[a], [i.Optional])
                }), {});
                return this.Object(l, u)
            }, s)
        }
        Rest(t) {
            return o.TTuple(t) && !I.IsUndefined(t.items) ? R.Rest(t.items) : o.TIntersect(t) ? R.Rest(t.allOf) : o.TUnion(t) ? R.Rest(t.anyOf) : []
        }
        String(t = {}) {
            return this.Create({
                ...t,
                [i.Kind]: "String",
                type: "string"
            })
        }
        TemplateLiteral(t, s = {}) {
            let u = I.IsString(t) ? me.Create(He.Parse(t)) : me.Create(t);
            return this.Create({
                ...s,
                [i.Kind]: "TemplateLiteral",
                type: "string",
                pattern: u
            })
        }
        Transform(t) {
            return new ge(t)
        }
        Tuple(t, s = {}) {
            let [u, l, T] = [!1, t.length, t.length], a = R.Rest(t), y = t.length > 0 ? {
                ...s,
                [i.Kind]: "Tuple",
                type: "array",
                items: a,
                additionalItems: u,
                minItems: l,
                maxItems: T
            } : {
                ...s,
                [i.Kind]: "Tuple",
                type: "array",
                minItems: l,
                maxItems: T
            };
            return this.Create(y)
        }
        Uncapitalize(t, s = {}) {
            return {
                ...X.Map(R.Type(t), "Uncapitalize"),
                ...s
            }
        }
        Union(t, s = {}) {
            return o.TTemplateLiteral(t) ? M.Resolve(t) : (() => {
                let u = t;
                if (u.length === 0) return this.Never(s);
                if (u.length === 1) return this.Create(R.Type(u[0], s));
                let l = R.Rest(u);
                return this.Create({
                    ...s,
                    [i.Kind]: "Union",
                    anyOf: l
                })
            })()
        }
        Unknown(t = {}) {
            return this.Create({
                ...t,
                [i.Kind]: "Unknown"
            })
        }
        Unsafe(t = {}) {
            return this.Create({
                ...t,
                [i.Kind]: t[i.Kind] || "Unsafe"
            })
        }
        Uppercase(t, s = {}) {
            return {
                ...X.Map(R.Type(t), "Uppercase"),
                ...s
            }
        }
    };
    i.JsonTypeBuilder = ae;
    var je = class extends ae {
        AsyncIterator(t, s = {}) {
            return this.Create({
                ...s,
                [i.Kind]: "AsyncIterator",
                type: "AsyncIterator",
                items: R.Type(t)
            })
        }
        Awaited(t, s = {}) {
            let u = l => l.length > 0 ? (() => {
                let [T, ...a] = l;
                return [this.Awaited(T), ...u(a)]
            })() : l;
            return o.TIntersect(t) ? i.Type.Intersect(u(t.allOf)) : o.TUnion(t) ? i.Type.Union(u(t.anyOf)) : o.TPromise(t) ? this.Awaited(t.item) : R.Type(t, s)
        }
        BigInt(t = {}) {
            return this.Create({
                ...t,
                [i.Kind]: "BigInt",
                type: "bigint"
            })
        }
        ConstructorParameters(t, s = {}) {
            return this.Tuple([...t.parameters], {
                ...s
            })
        }
        Constructor(t, s, u) {
            let [l, T] = [R.Rest(t), R.Type(s)];
            return this.Create({
                ...u,
                [i.Kind]: "Constructor",
                type: "Constructor",
                parameters: l,
                returns: T
            })
        }
        Date(t = {}) {
            return this.Create({
                ...t,
                [i.Kind]: "Date",
                type: "Date"
            })
        }
        Function(t, s, u) {
            let [l, T] = [R.Rest(t), R.Type(s)];
            return this.Create({
                ...u,
                [i.Kind]: "Function",
                type: "Function",
                parameters: l,
                returns: T
            })
        }
        InstanceType(t, s = {}) {
            return R.Type(t.returns, s)
        }
        Iterator(t, s = {}) {
            return this.Create({
                ...s,
                [i.Kind]: "Iterator",
                type: "Iterator",
                items: R.Type(t)
            })
        }
        Parameters(t, s = {}) {
            return this.Tuple(t.parameters, {
                ...s
            })
        }
        Promise(t, s = {}) {
            return this.Create({
                ...s,
                [i.Kind]: "Promise",
                type: "Promise",
                item: R.Type(t)
            })
        }
        RegExp(t, s = {}) {
            let u = I.IsString(t) ? t : t.source;
            return this.Create({
                ...s,
                [i.Kind]: "String",
                type: "string",
                pattern: u
            })
        }
        RegEx(t, s = {}) {
            return this.RegExp(t, s)
        }
        ReturnType(t, s = {}) {
            return R.Type(t.returns, s)
        }
        Symbol(t) {
            return this.Create({
                ...t,
                [i.Kind]: "Symbol",
                type: "symbol"
            })
        }
        Undefined(t = {}) {
            return this.Create({
                ...t,
                [i.Kind]: "Undefined",
                type: "undefined"
            })
        }
        Uint8Array(t = {}) {
            return this.Create({
                ...t,
                [i.Kind]: "Uint8Array",
                type: "Uint8Array"
            })
        }
        Void(t = {}) {
            return this.Create({
                ...t,
                [i.Kind]: "Void",
                type: "void"
            })
        }
    };
    i.JavaScriptTypeBuilder = je;
    i.JsonType = new ae;
    i.Type = new je
});
var Je = {
    en: "Xcode String Catalogs"
};
var _e = "An inlang plugin to handle Xcode String Catalogs";
var G = bn(Qe(), 1),
    Ye = G.Type.String({
        pattern: "^(\\./|\\../|/)[^*]*\\.xcstrings",
        title: "Path to xcstrings files",
        description: "Specify the file to read translations from. It must end with `.xcstrings`.",
        examples: ["./Localizable.xcstrings", "./ios/MyApp/Localizable.xcstrings"]
    }),
    Ze = G.Type.Object({
        file: G.Type.Union([Ye, G.Type.Record(G.Type.String({
            pattern: "^[^.]+$",
            description: "Dots are not allowed ",
            examples: ["myapp", "watch"]
        }), Ye)])
    });
async function Ge(c, t) {
    return JSON.parse(await t.readFile(c, {
        encoding: "utf-8"
    }))
}
var Nn = (c, t) => ({
    id: c,
    alias: {},
    selectors: [],
    variants: Object.entries(t).map(([s, u]) => ({
        languageTag: s,
        match: [],
        pattern: typeof u == "string" ? parsePattern(u) : u
    }))
}),
    Ae = "plugin.hechenbros.xcstrings";

/**
 * Parses a pattern. From inlang's JSON plugin.
 *
 * @example parsePattern("Hello {name}!", ["{", "}"])
 */
function parsePattern(
    text
) {
    // dependent on the variableReferencePattern, different regex
    // expressions are used for matching
    const expression = new RegExp(
        `(\\{[^\\}]+\\})`,
        "g"
    );

    const pattern = text
        .split(expression)
        .filter((element) => element !== "")
        .map((element) => {
            if (expression.test(element)) {
                return {
                    type: "VariableReference",
                    name: element.slice(
                        1,
                        // negative index, removing the trailing pattern
                        - 1
                    )
                };
            } else {
                return {
                    type: "Text",
                    value: element,
                };
            }
        });

    return pattern;
}


function* mn(c) {
    yield* c instanceof Object ? Object.entries(c).map(([s, u]) => ({
        key: s,
        pattern: u
    })) : [{
        key: void 0,
        pattern: c
    }]
}
async function* he(c, t) {
    for (let {
        key: s,
        pattern: u
    }
        of mn(t)) {
        let l = await Ge(u, c);
        yield {
            key: s,
            pattern: u,
            catalog: l,
            path: u
        }
    }
}
var en = " [using base]",
    nn = {
        id: Ae,
        displayName: Je,
        description: _e,
        settingsSchema: Ze,
        loadMessages: async ({
            settings: c,
            nodeishFs: t
        }) => {
            let {
                file: s
            } = c[Ae] ?? {};
            if (!s) throw new Error(`Missing 'file' property in settings for plugin ${Ae}`);
            let u = [];
            for await (let {
                key: l,
                catalog: T
            }
                of he(t, s)) {
                if (T.sourceLanguage !== c.sourceLanguageTag) throw new Error(`Source language '${T.sourceLanguage}' inside ${s} does not match project settings ('${c.sourceLanguageTag}')`);
                for (let [a, {
                    localizations: y
                }] of Object.entries(T.strings)) {
                    let f = [...l ? [l] : [], a].join(":");
                    u.push(Nn(f, Object.fromEntries([
                        [c.sourceLanguageTag, `${a}${en}`], ...Object.entries(y).map(([U, {
                            stringUnit: b
                        }]) => [U, b.value])
                    ])))
                }
            }
            return u
        },
        saveMessages: async ({
            messages: c,
            settings: t,
            nodeishFs: s
        }) => {
            let {
                file: u
            } = t[Ae] ?? {};
            for await (let {
                key: l,
                catalog: T,
                path: a
            }
                of he(s, u)) {
                let y = l ? `${l}:` : "";
                for (let f of c)
                    if (f.id.startsWith(y)) {
                        let U = f.id.slice(y.length);
                        T.strings[U] ??= {
                            localizations: {}
                        }, T.strings[U].localizations ??= {};
                        for (let b of f.variants) {
                            let O = b.pattern.map(m => m.value).join("");
                            if (b.languageTag === t.sourceLanguageTag && O === `${U}${en}`) {
                                console.log(`Skipping base localization for ${U}`);
                                continue
                            }
                            T.strings[U].localizations[b.languageTag] = {
                                stringUnit: {
                                    state: "translated",
                                    value: b.pattern.map(m => m.value).join("")
                                }
                            }
                        }
                    } await s.writeFile(a, JSON.stringify(T, null, 2).replaceAll(/": /g, '" : '))
            }
        }
    };
var En = nn;
export {
    En as
        default
};