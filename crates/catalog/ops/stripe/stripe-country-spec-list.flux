op stripe-country-spec-list(limit: Number) -> Any
  description "List country-specific requirements, supported currencies and payment capabilities without changing an account"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.stripe.com"
  url = fmt("{base}/v1/country_specs")
  sep = "?"
  when limit
    url = fmt("{url}{sep}limit={limit}")
  response = http.request(method: "GET", url)
  return response
