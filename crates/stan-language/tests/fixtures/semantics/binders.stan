functions { real square(real x) { return x * x; } } model { for (n in 1:2) { square(n); } }
