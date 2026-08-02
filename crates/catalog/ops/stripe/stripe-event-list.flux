op stripe-event-list(limit: Number) -> Any
  description "List Stripe account events for operational visibility without replaying or changing an event"
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.stripe.com"
  url = fmt("{base}/v1/events")
  sep = "?"
  when limit
    url = fmt("{url}{sep}limit={limit}")
  response = http.request(method: "GET", url)
  return response
