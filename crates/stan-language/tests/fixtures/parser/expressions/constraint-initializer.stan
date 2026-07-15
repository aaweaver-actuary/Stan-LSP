transformed parameters {
  real<lower=0, upper=1> theta = 0.5;
  array[N] real<lower=0> values = rep_array(0.0, N);
}
