op stripe-payment-intent-get(payment_intent: String) -> Any
  description "Get one payment intent by id — the modern shape of a payment, covering the whole lifecycle from creation to capture. Its `status` is what says where the payment stands; `requires_capture` means the money is authorized but not yet taken. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message`, its error code at `/error/code` in the response body."
  risk "low"
  idempotency "idempotent"
  effects ["network"]
  expose true

  base = "https://api.stripe.com"
  url = fmt("{base}/v1/payment_intents/{payment_intent}")
  response = http.request(method: "GET", url)
  return response
