op bitbucket-pull-request-comment(repo_slug: String, pull_request_id: Number, body: String) -> Any
  description "Add a top-level comment to a pull request. The comment is attributed to the account the token belongs to and notifies everyone participating; Bitbucket sends no un-notification, so a comment posted in error can be deleted but not un-seen. This posts a general comment on the pull request, never an inline comment on a line of the diff. A non-2xx response is returned as data, not a failure: the vendor's error message is at `/error/message` in the response body."
  risk "medium"
  idempotency "non_idempotent"
  effects ["write", "network"]
  expose true

  base = "https://api.bitbucket.org/2.0"
  workspace = "{workspace}"
  url = fmt("{base}/repositories/{workspace}/{repo_slug}/pullrequests/{pull_request_id}/comments")
  content_type = "application/json"
  payload = { content: { raw: body } }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
