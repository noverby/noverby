const VideoViewer = ({ file }: { file?: string }) =>
  file ? (
    <video autoPlay width="100%" controls>
      <source src={file} />
    </video>
  ) : null;

export default VideoViewer;
