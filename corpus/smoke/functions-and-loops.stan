functions {
  real square(real x) {
    return x * x;
  }
}
data {
  int<lower=0> N;
}
generated quantities {
  array[N] real values;
  for (n in 1:N) {
    values[n] = square(n);
  }
}
