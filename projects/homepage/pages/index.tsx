import dynamic from "next/dynamic";
import { useEffect, useState } from "react";
import Graph from "../components/Graph"
import Head from "next/head";

export default function Index() {
  const [showing, setShowing] = useState(false);
  useEffect(() => {
    setShowing(true);
  }, []);
  return (
    <>
      <Head><title>{"Niclas Overby Ⓝ"}</title></Head>
      {showing && <Graph />}
      <a rel="me" href="https://mas.to/@niclasoverby" />
    </>
  );
}
