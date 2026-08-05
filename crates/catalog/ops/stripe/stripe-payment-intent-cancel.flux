op stripe-payment-intent-cancel(payment_intent: String, idempotency_key: String) -> Any
  description "Cancel a payment intent, releasing any authorization hold on the customer's card. This cannot be undone — a canceled intent is final, and collecting the payment afterwards means creating a new one. An intent that has already succeeded cannot be canceled; refund it instead. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message`, its error code at `/error/code` in the response body."
  risk "high"
  idempotency "conditional"
  effects ["write", "network"]
  expose true

  base = "https://api.stripe.com"
  url = fmt("{base}/v1/payment_intents/{payment_intent}/cancel")
  response = http.request(headers: { "Idempotency-Key": idempotency_key }, method: "POST", url)
  return response
