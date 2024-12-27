// import React, { useEffect } from "react";
// import {
//   BarSeries,
//   Chart as DxChart,
//   ValueAxis,
//   Tooltip,
//   Legend,
// } from "@devexpress/dx-react-chart-material-ui";
// import {
//   Animation,
//   ValueScale,
//   EventTracker,
//   HoverState,
//   Stack,
// } from "@devexpress/dx-react-chart";
// import {
//   Card,
//   CardHeader,
//   Typography,
//   Box,
//   CardContent,
// } from "@mui/material";
// import { Node, useScreen, useSession } from "hooks";
// import { nodes, Maybe, useSubscription, String_comparison_exp } from "gql";
// import { MimeAvatarId } from "comps";

// const MuiChart = DxChart as any;

// const Chart = ({ chartData, title }: { chartData: any; title: string }) => {
//   if (chartData.options.length === 2) return null;
//   return (
//     <Card sx={{ m: 1 }}>
//       <CardHeader
//         title={title}
//         avatar={<MimeAvatarId id={chartData.nodeId} />}
//       />
//       {chartData?.options?.map((opt: any, index: number) => (
//         <Box key={index} />
//       ))}
//       <MuiChart data={chartData.data} rotated>
//         <ValueAxis />
//         <ValueScale
//           modifyDomain={(domain: any) => {
//             return [1, 20];
//           }}
//         />

//         {chartData.options?.map((opt: string, index: number) => (
//           <BarSeries
//             name={opt}
//             valueField={`${index}`}
//             argumentField="arg"
//             key={index}
//           />
//         ))}
//         <Animation />
//         <EventTracker />
//         <Tooltip />
//         <HoverState />
//         <Legend position="bottom" />
//         <Stack />
//       </MuiChart>
//       {/*
//       <CardContent>
//         <Typography>{`${count} / ${voters} stemmer`}</Typography>
//       </CardContent> */}
//     </Card>
//   );
// };

// export default PollChartSub;
// const PollChartSub = ({ node }: { node: Node }) => {
//   const screen = useScreen();
//   const poll = node.useSubs();

//   // Todo proper detection
//   //const voters = poll?.context?.members_aggregate().aggregate?.count() ?? 11;

//   //const voteCount = group?.sub?.members_aggregate().aggregate?.count()
//   //poll?.context?.mimes({ where: { name: { _eq: "vote/vote" } }) members_aggregate({ where: { parent: { permissions: { _and: [{ mimeId: { _eq: "vote/vote" } } ] } } }}).aggregate?.count
//   // const voters = poll?.context?.mimes({
//   //   where: {
//   //     _and: [
//   //       { name: { _eq: "vote/vote" } },
//   //       {
//   //         permisssions: {
//   //           _and: [
//   //             { insert: { _eq: true } },
//   //             { node: { members: { active: { _eq: true } } } },
//   //           ],
//   //         },
//   //       },
//   //     ],
//   //   },
//   // });
//   const count = poll
//     ?.children_aggregate({ where: { mimeId: { _eq: "vote/vote" } } })
//     .aggregate?.count();
//   const data = poll?.data();

//   const owner = poll?.isContextOwner;
//   const mutable = poll?.mutable;
//   const options = data?.options ?? [];
//   const hidden = data?.hidden ?? true;
//   const nodeId = data?.nodeId;

//   const opts = [...Array(options.length).keys()].map((opt: number) => [opt, 1]);
//   const votes = poll?.children({
//     where: { mimeId: { _eq: "vote/vote" } },
//   });
//   const acc = votes
//     ?.map(({ data }) => data())
//     .flat()
//     .reduce((acc, e) => acc.set(e, acc.get(e) + 2), new Map(opts as any));
//   const res = { arg: "none", ...[...(acc?.values() ?? []), count] };

//   const chartData =
//     mutable || (hidden && (screen || !owner))
//       ? {
//           options: [
//             ...options.map((opt: string) => `${opt} (skjult)`),
//             "Antal Stemmer",
//           ],
//           data: [
//             {
//               arg: "none",
//               ...[...Array(options.length).keys()].map((opt: number) => 1),
//               [options.length]: count,
//             },
//           ],
//           nodeId,
//         }
//       : { options: [...options, "Antal Stemmer"], data: [res], nodeId };

//   const title = poll?.name;

//   return <Chart chartData={chartData} title={title ?? ""} />;
// }
export {};
