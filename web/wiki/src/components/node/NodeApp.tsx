// TODO: find library with smaller data footprint
export {};
/*
import { renderToString } from "react-dom/server";
import { resolve, useQuery } from "gql";
import { getIcon } from "mime";
import { useEffect, useState } from "react";
import { fromId, toWhere } from "core/path";
import { useRouter } from "next/navigation";
//import { ForceGraph2D } from "react-force-graph";
const { ForceGraph2D } =
  typeof window !== "undefined"
    ? require("react-force-graph")
    : { ForceGraph2D: null };

type Node = { id?: string; name?: string; group?: string, color?: string };
type Link = { source?: string; target?: string; value?: number };
type GraphData = {
  nodes: Node[];
  links: Link[];
};

const createNodeLabel = (node: any) => {
  const { id, name, group, mime } = node;
  const jsx = getIconFromId({ name: mime });
  return renderToString(jsx);
};

const uniqBy = <T,>(a: T[], key: (v: T) => string) => {
  const seen = new Set();
  return a.filter(item => {
    const k = key(item);
    return seen.has(k) ? false : seen.add(k);
  });
}

export default NodeApp;
const NodeApp = ({ node }: { node: Node }) => {
  const router = useRouter();
  const query = useQuery();
  const [dim, setDim] = useState({
    width: window.innerWidth,
    height: window.innerHeight,
  });
  const [data, setData] = useState<GraphData>({ nodes: [], links: [] });
  const [cache, setCache] = useState<GraphData>({ nodes: [], links: [] });

  useEffect(() => {
    const handleResize = () => {
      setDim({ width: window.innerWidth, height: window.innerHeight });
    };

    window.addEventListener("resize", handleResize);
    return () => window.removeEventListener("resize", handleResize);
  }, []);

  useEffect(() => {
    const currentNode = [{ id: node?.id, name: node?.name, mime: node?.mimeId, group: node?.mimeId }];
    const parentNode = [{
      id: node?.parent?.id,
      name: node?.parent?.name,
      mime: node?.parent?.mimeId,
      group: node?.parent?.mimeId,
    }];
    const parentLink = [{ source: node?.id, target: node?.parent?.id, value: 4 }];
    const childNodes =
      node?.children().map(({ id, name, mime }) =>
      ({
        id,
        name,
        mime: mimeId,
        group: mimeId,
      })
      ) ?? [];
    const childLinks =
      node
        ?.children()
        .map(({ id }) =>
          ({ source: node?.id, target: id, value: 5 })
        ) ?? [];

    const newData = {
      nodes: uniqBy([...currentNode, ...cache.nodes, ...parentNode, ...childNodes], v => v.id),
      links: uniqBy([...cache.links, ...parentLink, ...(childLinks?.[0]?.target ? childLinks : [])], v => v.source > v.target ? v.source + v.target : v.target + v.source),
    };
    //node?.relations().map((rel) => {
    //  addNode({ id: rel?.id, content: rel?.name, coordinates: [0, 0] });
    //  connect(node?.id, rel?.id)
    //});
    if (node?.id) {
      setCache(JSON.parse(JSON.stringify(newData)));
      setData(newData);
    }
  }, [node]);

  if (typeof window === "undefined") return null;

  const handleOnClick = async (node: Node) => {
    const path = await fromId(node?.id);
    await resolve(({ query }) => {
      const { id, name, mimeId } = query.nodes(toWhere(path))?.[0];
    })

    router.push(`/${path.join("/")}${node.id !== id ? "?app=node" : ""}`);
  }

  return (
    <ForceGraph2D
      width={dim.width > 1150 ? 1150 : dim.width - 50}
      height={dim.height - 240}
      graphData={data}
      nodeAutoColorBy="group"
      nodeCanvasObject={(node: any, ctx: any, globalScale: number) => {
        const label = node.name;
        const fontSize = 14 / globalScale;
        ctx.font = `${fontSize}px Sans-Serif`;
        const textWidth = ctx.measureText(label).width;
        const bckgDimensions = [textWidth, fontSize].map(n => n + fontSize * 0.2); // some padding

        ctx.fillStyle = 'rgba(255, 255, 255, 0.8)';
        ctx.fillRect(node.x - bckgDimensions[0] / 2, node.y - bckgDimensions[1] / 2, ...bckgDimensions);

        ctx.textAlign = 'center';
        ctx.textBaseline = 'middle';
        ctx.fillStyle = node.color;
        ctx.fillText(label, node.x, node.y);

        node.__bckgDimensions = bckgDimensions; // to re-use in nodePointerAreaPaint
      }}
      nodePointerAreaPaint={(node: any, color: any, ctx: any) => {
        ctx.fillStyle = color;
        const bckgDimensions = node.__bckgDimensions;
        bckgDimensions && ctx.fillRect(node.x - bckgDimensions[0] / 2, node.y - bckgDimensions[1] / 2, ...bckgDimensions);
      }}
      nodeLabel={createNodeLabel}
      onNodeClick={handleOnClick}
    />
  );
}
*/
