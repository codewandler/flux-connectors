op stripe-exchange-rate-list(limit: Number) -> Any
  description "List current Stripe exchange rates without creating a conversion or moving funds"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.stripe.com"
  url = fmt("{base}/v1/exchange_rates")
  sep = "?"
  when limit
    url = fmt("{url}{sep}limit={limit}")
  response = http.request(method: "GET", url)
  return response
