functions {
  real custom_lpdf(real y, real theta) {
    return normal_lpdf(y | theta, 1);
  }
}
data {
  real y;
}
parameters {
  real theta;
}
model {
  y ~ custom(theta);
}
