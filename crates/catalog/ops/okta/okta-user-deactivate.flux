op okta-user-deactivate(user_id: String, send_email: Bool) -> Any
  description "Deactivate a user, moving them to DEPROVISIONED. This takes effect immediately: Okta ends the person's active sessions, revokes their tokens and grants, and removes their access to every application the org assigns through Okta. They cannot sign in afterwards. An administrator can reactivate the account later, but that is a separate operation this connector does not offer, and it does not restore the sessions or tokens this call destroyed"
  risk "destructive"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://{domain}/api/v1"
  url = fmt("{base}/users/{user_id}/lifecycle/deactivate")
  response = http.request(method: "POST", query: { sendEmail: send_email }, url)
  return response
