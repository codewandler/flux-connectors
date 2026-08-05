op stripe-payment-intent-capture(payment_intent: String, idempotency_key: String) -> Any
  description "Capture an authorized payment intent, charging the customer the **full** authorized amount. Only a payment intent in `requires_capture` can be captured; a partial capture needs an `amount` this connector cannot send. Stripe answers 402 with an `error.code` such as `card_declined` when the capture is refused, and that arrives as data rather than as a failure. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message`, its error code at `/error/code` in the response body."
  risk "destructive"
  idempotency "conditional"
  effects ["write", "network"]
  expose true

  base = "https://api.stripe.com"
  url = fmt("{base}/v1/payment_intents/{payment_intent}/capture")
  response = http.request(headers: { "Idempotency-Key": idempotency_key }, method: "POST", url)
  return response
