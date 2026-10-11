package kabam.rotmg.messaging.impl.outgoing
{
import flash.utils.IDataOutput;

// How far (in tiles) the client's view reaches.
public class ViewRadius extends OutgoingMessage
{
    public var radius_:int;

    public function ViewRadius(id:uint, callback:Function)
    {
        super(id,callback);
    }

    override public function writeToOutput(data:IDataOutput) : void
    {
        data.writeInt(this.radius_);
    }

    override public function toString() : String
    {
        return formatToString("VIEW_RADIUS","radius_");
    }
}
}
