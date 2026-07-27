model {
  for (n in 1:2) {
    y ~ normal(mu[n], sigma);
    print(y);
  }
}
