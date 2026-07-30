op stripe-balance-get -> Any
  description "Get the account's current balance — what is available to pay out and what is still pending, per currency. Takes no parameters, and reports the balance of whichever mode the key belongs to: a test key returns the test balance. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message`, its error code at `/error/code` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.stripe.com"
  url = fmt("{base}/v1/balance")
  response = http.request(method: "GET", url)
  return response
