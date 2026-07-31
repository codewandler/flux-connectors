op linear-comment-create(issueId: String, body: String) -> Any
  description "Add a comment to an issue. The comment is attributed to the user the API key belongs to and notifies everyone watching the issue; Linear sends no un-notification, so a comment posted in error can be deleted but not un-seen. Linear answers every failure with HTTP 200 and an `errors` array beside a null `data`, so check `errors` and the payload's `success` flag before treating the comment as posted"
  risk "medium"
  idempotency "non_idempotent"
  effects ["network"]
  expose true

  base = "https://api.linear.app"
  url = fmt("{base}/graphql")
  content_type = "application/json"
  query = """mutation CommentCreate($issueId: String!, $body: String!) {
  commentCreate(input: {issueId: $issueId, body: $body}) {
    success
    comment {
      id
      url
      createdAt
    }
  }
}
"""
  payload = { query, variables: { body, issueId } }
  response = http.request(body: payload, headers: { "content-type": content_type }, method: "POST", url)
  return response
