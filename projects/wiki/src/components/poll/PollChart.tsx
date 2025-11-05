// import React from "react";
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
// import { Card, CardHeader } from "@mui/material";
// import { nodes, Maybe } from "gql";
// import { Node, useNode } from "hooks";

// const Chart = DxChart as any;

// const parseData = (poll: Maybe<nodes> | undefined, screen: boolean) => {
//   const count = poll
//     ?.children_aggregate({ where: { mimeId: { _eq: "vote/vote" } } })
//     .aggregate?.count();
//   const data = poll?.data();

//   const owner = poll?.isContextOwner;
//   const mutable = poll?.mutable;
//   const { options, hidden } =
//     data && poll?.mimeId == "vote/poll"
//       ? data
//       : { options: [], hidden: true };

//   const opts = [...Array(options.length).keys()].map((opt: number) => [opt, 0]);
//   const votes = poll?.children({
//     where: { mimeId: { _eq: "vote/vote" } },
//   });
//   // TODO: validate
//   const acc = votes
//     ?.map(({ data }) => data())
//     .flat()
//     .reduce((acc, e) => acc.set(e, acc.get(e) + 1), new Map(opts as any));
//   const res = { arg: "none", ...[...acc.values()] };

//   if (mutable || (hidden && (screen || !owner))) {
//     return {
//       options: ["Antal Stemmer"],
//       data: [{ arg: "none", 0: count }],
//     };
//   }
//   return { options, data: [res] };
// };

// export default PollChart;
// const PollChart = ({
//   node,
//   screen = false,
// }: {
//   node: Node;
//   screen?: boolean;
// }) => {
//   const poll = node.useQuery();

//   const title = poll?.name;
//   const chartData = parseData(poll, screen);

//   // isContextOwner is undefined during first fetch
//   if (!chartData.options.length) return null;

//   return (
//     <Card sx={{ m: 0 }}>
//       <CardHeader
//         sx={{
//           bgcolor: "secondary.main",
//           color: (t) => t.palette.secondary.contrastText,
//         }}
//         title={title}
//       />
//       <Chart data={chartData.data} rotated>
//         <ValueAxis />
//         <ValueScale
//           modifyDomain={(domain: any) => {
//             //return [0, voteCount ? voteCount : 20];
//             return [0, 20];
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
//       </Chart>
//     </Card>
//   );
// }
export {};
