<div align="center">

# hello-rust

<details>
<summary>
Deployed to ??!?!?!
</summary>

https://eu-west-2.console.aws.amazon.com/dynamodbv2/home?region=eu-west-2#item-explorer?initialTagKey=&table=tasks

</details>

[![Rust](https://github.com/SG60/hello-rust/actions/workflows/rust.yml/badge.svg)](https://github.com/SG60/hello-rust/actions/workflows/rust.yml)

</div>

---

Uses Just (see the justfile) for running scripts etc.


https://eu-west-2.console.aws.amazon.com/dynamodbv2/home?region=eu-west-2#item-explorer?initialTagKey=&table=tasks

# DB Design

https://docs.rs/aws-sdk-dynamodb/latest/aws_sdk_dynamodb/
https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/GettingStarted.html
https://github.com/awslabs/aws-sdk-rust/tree/main/examples/dynamodb
https://docs.aws.amazon.com/sdk-for-rust/latest/dg/getting-started.html
https://docs.aws.amazon.com/amazondynamodb/latest/developerguide/best-practices.html

The DB just stores sync settings and refresh tokens.

<table>
<caption>DB Example</caption>
<tr>
        <th scope="col" colspan=2>PK</th>
        <th scope="col" colspan=99999>Attributes</th>
</tr>
<tbody>
	<tr><td rowSpan=0>notionUserId
	<tr><td rowspan=2>details<th>googleRefreshToken<th>notionRefreshToken<th>googleUserId
		<tr><td>abcdsafeeasf<td>c<td>user123456
	<tr><td rowspan=2>sync#0<th>notionDatabase<th>googleCalendar<th>notionTitleId<th>notionDoneId
		<tr><td>1234<td>1234<td>asdf3<td>flkjhs
	<tr><td rowspan=2>sync#1<th>notionDatabase<th>googleCalendar
		<tr><td>1234<td>1234
</tbody>
</table>

Probably doesn't need rows for items. Maybe they can all just be stored in memory.

https://eu-west-2.console.aws.amazon.com/dynamodbv2/home?region=eu-west-2#item-explorer?initialTagKey=&table=tasks
