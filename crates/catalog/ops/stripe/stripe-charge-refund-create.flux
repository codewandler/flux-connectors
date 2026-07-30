op stripe-charge-refund-create(charge: String, idempotency_key: String) -> Any
  description "Refund a charge **in full** and irreversibly: the entire un-refunded amount goes back to the customer's original payment method, usually within five to ten business days. A partial refund needs an `amount` this connector cannot send. Stripe's fee on the original charge is not returned. There is no way to undo a refund — collecting the money again means charging the customer again. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message`, its error code at `/error/code` in the response body."
  risk "destructive"
  idempotency "conditional"
  effects ["network"]
  expose true

  base = "https://api.stripe.com"
  url = fmt("{base}/v1/charges/{charge}/refunds")
  response = http.request(headers: { "Idempotency-Key": idempotency_key }, method: "POST", url)
  return response
