op stripe-country-spec-list(limit: Number) -> Any
  description "List country-specific requirements, supported currencies and payment capabilities without changing an account"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.stripe.com"
  url = fmt("{base}/v1/country_specs")
  response = http.request(method: "GET", query: { limit }, url)
  return response
