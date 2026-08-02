op stripe-billing-meter-list(limit: Number) -> Any
  description "List usage-billing meter definitions without creating, changing or deactivating one"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.stripe.com"
  url = fmt("{base}/v1/billing/meters")
  sep = "?"
  when limit
    url = fmt("{url}{sep}limit={limit}")
  response = http.request(method: "GET", url)
  return response
