op stripe-exchange-rate-list(limit: Number) -> Any
  description "List current Stripe exchange rates without creating a conversion or moving funds"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.stripe.com"
  url = fmt("{base}/v1/exchange_rates")
  response = http.request(method: "GET", query: { limit }, url)
  return response
