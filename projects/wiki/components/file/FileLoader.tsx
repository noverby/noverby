import { Node, useFile } from 'hooks';
import {
  VideoViewer,
  ImageViewer,
  MsOfficeViewer,
  AudioViewer,
  PdfViewer,
} from 'comps';

const FileLoader = ({ node }: { node: Node }) => {
  const data = node.useQuery()?.data();
  const file = useFile({ fileId: data?.fileId });

  switch (data?.type) {
    case 'application/pdf':
      return <PdfViewer file={file} />;
    case 'image/bmp':
    case 'image/jpg':
    case 'image/jpeg':
    case 'image/png':
    case 'image/tif':
    case 'image/tiff':
      return <ImageViewer file={file} />;
    case 'audio/ogg':
      return <AudioViewer file={file} type={data?.type} />;
    case 'video/mp4':
      return <VideoViewer file={file} />;
    case 'application/msword':
    case 'application/vnd.openxmlformats-officedocument.wordprocessingml.document':
    case 'application/vnd.ms-excel':
    case 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet':
    case 'application/vnd.ms-powerpoint':
    case 'application/vnd.openxmlformats-officedocument.presentationml.presentation':
      return <MsOfficeViewer file={file} />;
    default:
  }

  return null;
};

export default FileLoader;
