// Evaluates gettext `Plural-Forms` expressions (C syntax over `n`) without eval.

type Expr = (n: number) => number;

export function compilePlural(source: string): Expr {
  const tokens = source.match(/\d+|n|&&|\|\||[=!<>]=|[-+*/%<>?:()!]/g) ?? [];
  let pos = 0;
  const peek = () => tokens[pos];
  const next = () => tokens[pos++];

  // Operator precedence levels, lowest first.
  const BINARY = [["||"], ["&&"], ["==", "!="], ["<", "<=", ">", ">="], ["+", "-"], ["*", "/", "%"]];
  const apply = (op: string, a: number, b: number): number => {
    switch (op) {
      case "||": return +(a !== 0 || b !== 0);
      case "&&": return +(a !== 0 && b !== 0);
      case "==": return +(a === b);
      case "!=": return +(a !== b);
      case "<": return +(a < b);
      case "<=": return +(a <= b);
      case ">": return +(a > b);
      case ">=": return +(a >= b);
      case "+": return a + b;
      case "-": return a - b;
      case "*": return a * b;
      case "/": return b === 0 ? 0 : Math.trunc(a / b);
      default: return b === 0 ? 0 : a % b;
    }
  };

  function ternary(): Expr {
    const cond = binary(0);
    if (peek() !== "?") return cond;
    next();
    const yes = ternary();
    if (next() !== ":") throw new Error("expected ':'");
    const no = ternary();
    return (n) => (cond(n) !== 0 ? yes(n) : no(n));
  }

  function binary(level: number): Expr {
    if (level === BINARY.length) return unary();
    let left = binary(level + 1);
    while (BINARY[level].includes(peek() ?? "")) {
      const op = next()!;
      const lhs = left;
      const rhs = binary(level + 1);
      left = (n) => apply(op, lhs(n), rhs(n));
    }
    return left;
  }

  function unary(): Expr {
    const token = next();
    if (token === "!") {
      const inner = unary();
      return (n) => +(inner(n) === 0);
    }
    if (token === "(") {
      const inner = ternary();
      if (next() !== ")") throw new Error("expected ')'");
      return inner;
    }
    if (token === "n") return (n) => n;
    if (token !== undefined && /^\d+$/.test(token)) {
      const value = Number(token);
      return () => value;
    }
    throw new Error(`unexpected token ${token}`);
  }

  try {
    const expr = ternary();
    if (pos !== tokens.length) throw new Error("trailing tokens");
    return expr;
  } catch {
    return (n) => +(n !== 1);
  }
}
