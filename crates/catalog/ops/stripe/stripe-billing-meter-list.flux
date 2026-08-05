op stripe-billing-meter-list(limit: Number) -> Any
  description "List usage-billing meter definitions without creating, changing or deactivating one"
  risk "low"
  idempotency "idempotent"
  effects ["read", "network"]
  expose true

  base = "https://api.stripe.com"
  url = fmt("{base}/v1/billing/meters")
  response = http.request(method: "GET", query: { limit }, url)
  return response
