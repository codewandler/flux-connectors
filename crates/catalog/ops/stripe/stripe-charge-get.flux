op stripe-charge-get(charge: String) -> Any
  description "Get one charge by id: amount, currency, whether it was captured, whether it was refunded and how much of it, the card's last four digits and brand, and the failure or decline reason if it did not succeed. `amount` is in the currency's smallest unit — 1000 is ten dollars, and also ten thousand yen. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message`, its error code at `/error/code` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.stripe.com"
  url = fmt("{base}/v1/charges/{charge}")
  response = http.request(method: "GET", url)
  return response
