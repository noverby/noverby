const AudioViewer = ({ file, type }: { file?: string; type: string }) =>
  file ? (
    <audio autoPlay controls>
      <source src={file} />
    </audio>
  ) : null;

export default AudioViewer;
